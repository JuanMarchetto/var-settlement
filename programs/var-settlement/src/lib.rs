//! VAR — Verifiable Automated Resolution
//!
//! A trustless settlement engine for FIFA World Cup 1X2 markets. It escrows USDC in a parimutuel
//! pool, resolves each market by authenticating the final scoreline against Tx LINE's on-chain
//! `daily_scores_roots` Merkle feed (CPI into `Txoracle::validate_stat`), maps the authenticated
//! facts to an outcome with the **Kani-proven `rulebook`**, writes a proof-carrying receipt, and
//! exposes a permissionless `reverify` so anyone can re-derive the resolution in one transaction.
//!
//! No token vote. No dispute bond. No trusted arbiter. Settlement in USDC only — never the TxL token.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use solana_program::keccak::hashv;
use rulebook::{resolve as rulebook_resolve, MatchState, MatchStatus, Outcome, Pools};

declare_id!("AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk");

/// Bump the ruleset hash when resolution semantics change, so receipts bind to a specific ruleset.
pub const RULESET_VERSION: u8 = 1;
pub const MARKET_KIND_1X2: u8 = 0;

// Outcome wire codes (kept in sync with `rulebook::Outcome`).
const OUT_HOME: u8 = 0;
const OUT_DRAW: u8 = 1;
const OUT_AWAY: u8 = 2;
const OUT_REFUND: u8 = 3;

#[program]
pub mod var_settlement {
    use super::*;

    /// Create a 1X2 market for one fixture. `home_stat_key`/`away_stat_key`/`period` bind which
    /// authenticated Tx LINE stats count as the regulation home/away goals at resolution.
    pub fn create_market(ctx: Context<CreateMarket>, args: CreateMarketArgs) -> Result<()> {
        // `market_kind` doubles as a per-fixture nonce (all values are 1X2 markets in V1), so a
        // fixture can host more than one distinct market account.
        require!(args.fee_bps <= rulebook::MAX_FEE_BPS, VarError::FeeTooHigh);
        require!(args.resolve_deadline > Clock::get()?.unix_timestamp, VarError::DeadlineInPast);

        let m = &mut ctx.accounts.market;
        m.fixture_id = args.fixture_id;
        m.market_kind = args.market_kind;
        m.home_stat_key = args.home_stat_key;
        m.away_stat_key = args.away_stat_key;
        m.period = args.period;
        m.fee_bps = args.fee_bps;
        m.resolve_deadline = args.resolve_deadline;
        m.status = MarketStatus::Open as u8;
        m.usdc_mint = ctx.accounts.usdc_mint.key();
        m.vault = ctx.accounts.vault.key();
        m.pool_home = 0;
        m.pool_draw = 0;
        m.pool_away = 0;
        m.pending_home_goals = 0;
        m.pending_home_set = false;
        m.home_events_root = [0u8; 32];
        m.receipt = ResolutionReceipt::default();
        m.bump = ctx.bumps.market;
        Ok(())
    }

    /// Stake USDC on one outcome. Escrows into the market vault and records the position.
    pub fn deposit(ctx: Context<Deposit>, outcome: u8, amount: u64) -> Result<()> {
        let m = &mut ctx.accounts.market;
        require!(m.status == MarketStatus::Open as u8, VarError::MarketNotOpen);
        require!(Clock::get()?.unix_timestamp < m.resolve_deadline, VarError::MarketClosed);
        require!(amount > 0, VarError::ZeroAmount);
        require!(outcome <= OUT_AWAY, VarError::InvalidOutcome);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.depositor_ata.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            amount,
        )?;

        let p = &mut ctx.accounts.position;
        if p.owner == Pubkey::default() {
            p.owner = ctx.accounts.depositor.key();
            p.market = m.key();
            p.bump = ctx.bumps.position;
        }
        match outcome {
            OUT_HOME => {
                m.pool_home = m.pool_home.checked_add(amount).ok_or(VarError::MathOverflow)?;
                p.stake_home = p.stake_home.checked_add(amount).ok_or(VarError::MathOverflow)?;
            }
            OUT_DRAW => {
                m.pool_draw = m.pool_draw.checked_add(amount).ok_or(VarError::MathOverflow)?;
                p.stake_draw = p.stake_draw.checked_add(amount).ok_or(VarError::MathOverflow)?;
            }
            OUT_AWAY => {
                m.pool_away = m.pool_away.checked_add(amount).ok_or(VarError::MathOverflow)?;
                p.stake_away = p.stake_away.checked_add(amount).ok_or(VarError::MathOverflow)?;
            }
            _ => return err!(VarError::InvalidOutcome),
        }
        Ok(())
    }

    /// Step 1 of resolution (permissionless): authenticate the final HOME goals against Tx LINE's
    /// on-chain Merkle root (CPI `validate_stat`, `EqualTo` predicate) and cache them. Split from
    /// `resolve` because both Merkle proofs together exceed the 1232-byte transaction limit.
    pub fn attest_home(ctx: Context<Resolve>, home: StatWitness) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let m = &mut ctx.accounts.market;
        require!(m.status == MarketStatus::Open as u8, VarError::AlreadySettled);
        require!(now <= m.resolve_deadline.saturating_add(RESOLVE_GRACE_SECS), VarError::ResolveWindowPassed);
        require!(home.summary.fixture_id == m.fixture_id, VarError::FixtureMismatch);
        require!(home.stat.stat_to_prove.key == m.home_stat_key, VarError::StatKeyMismatch);
        require!(home.stat.stat_to_prove.period == m.period, VarError::StatPeriodMismatch);

        let home_goals = home.stat.stat_to_prove.value;
        let ok = txoracle_cpi::validate_stat_equal(
            &ctx.accounts.txoracle_program,
            &ctx.accounts.daily_scores_merkle_roots,
            &home,
            home_goals,
        )?;
        require!(ok, VarError::StatNotAuthenticated);

        m.pending_home_goals = home_goals;
        m.pending_home_set = true;
        m.home_events_root = home.summary.events_sub_tree_root;
        Ok(())
    }

    /// Step 2 (permissionless): authenticate the AWAY goals, then run the Kani-proven rulebook on
    /// the cached home goals + authenticated away goals and write a proof-carrying receipt.
    pub fn resolve(ctx: Context<Resolve>, away: StatWitness, status_code: u8) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let m = &mut ctx.accounts.market;
        require!(m.status == MarketStatus::Open as u8, VarError::AlreadySettled);
        require!(m.pending_home_set, VarError::HomeNotAttested);
        require!(now <= m.resolve_deadline.saturating_add(RESOLVE_GRACE_SECS), VarError::ResolveWindowPassed);

        // Bind the away witness to THIS market's fixture + configured stat key/period.
        require!(away.summary.fixture_id == m.fixture_id, VarError::FixtureMismatch);
        require!(away.stat.stat_to_prove.key == m.away_stat_key, VarError::StatKeyMismatch);
        require!(away.stat.stat_to_prove.period == m.period, VarError::StatPeriodMismatch);

        let away_goals = away.stat.stat_to_prove.value;
        let ok_away = txoracle_cpi::validate_stat_equal(
            &ctx.accounts.txoracle_program,
            &ctx.accounts.daily_scores_merkle_roots,
            &away,
            away_goals,
        )?;
        require!(ok_away, VarError::StatNotAuthenticated);

        // Run the formally-verified rulebook on the authenticated facts.
        let home_goals = m.pending_home_goals;
        let status = decode_status(status_code)?;
        let pools = Pools::new(m.pool_home, m.pool_draw, m.pool_away);
        let r = rulebook_resolve(&MatchState::new(home_goals, away_goals, status), pools, m.fee_bps);

        // Provenance: bind the receipt to both authenticated sub-tree roots and the ruleset.
        let source_root = hashv(&[
            m.home_events_root.as_slice(),
            away.summary.events_sub_tree_root.as_slice(),
        ])
        .to_bytes();
        let ver = [RULESET_VERSION];
        let fee = m.fee_bps.to_le_bytes();
        let hk = m.home_stat_key.to_le_bytes();
        let ak = m.away_stat_key.to_le_bytes();
        let per = m.period.to_le_bytes();
        let ruleset_hash = hashv(&[
            ver.as_slice(),
            fee.as_slice(),
            hk.as_slice(),
            ak.as_slice(),
            per.as_slice(),
        ])
        .to_bytes();

        m.receipt = ResolutionReceipt {
            source_root,
            ruleset_hash,
            home_goals,
            away_goals,
            status_code,
            outcome_code: encode_outcome(r.outcome),
            paid_as_refund: r.settlement.paid_as_refund,
            pot: r.settlement.pot,
            fee: r.settlement.fee,
            net: r.settlement.net,
            winning_pool: r.settlement.winning_pool,
            resolved_ts: now,
        };
        m.status = MarketStatus::Settled as u8;

        emit!(MarketResolved {
            market: m.key(),
            fixture_id: m.fixture_id,
            outcome_code: m.receipt.outcome_code,
            paid_as_refund: m.receipt.paid_as_refund,
            home_goals,
            away_goals,
        });
        Ok(())
    }

    /// Claim a winning payout or a refund. Idempotent per position (single-claim guard).
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let m = &ctx.accounts.market;
        require!(m.status == MarketStatus::Settled as u8, VarError::NotSettled);
        let p = &mut ctx.accounts.position;
        require!(!p.claimed, VarError::AlreadyClaimed);

        let rc = &m.receipt;
        let amount = if rc.paid_as_refund {
            p.stake_home
                .checked_add(p.stake_draw)
                .and_then(|x| x.checked_add(p.stake_away))
                .ok_or(VarError::MathOverflow)?
        } else {
            let (winning_stake, winning_pool) = match rc.outcome_code {
                OUT_HOME => (p.stake_home, rc.winning_pool),
                OUT_DRAW => (p.stake_draw, rc.winning_pool),
                OUT_AWAY => (p.stake_away, rc.winning_pool),
                _ => (0, 0),
            };
            let s = rulebook::Settlement {
                paid_as_refund: false,
                pot: rc.pot,
                fee: rc.fee,
                net: rc.net,
                winning_pool,
            };
            rulebook::winner_payout(&s, winning_stake)
        };

        p.claimed = true;
        if amount == 0 {
            return Ok(());
        }

        let market_key = m.key();
        let seeds: &[&[u8]] = &[b"vault", market_key.as_ref(), &[ctx.bumps.vault_authority]];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.recipient_ata.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                &[seeds],
            ),
            amount,
        )?;
        Ok(())
    }

    /// Permissionless re-derivation: re-run the rulebook from the stored receipt and assert it
    /// still yields the recorded outcome and settlement. Returns `true` on match. Read-only (`.view`).
    pub fn reverify(ctx: Context<Reverify>) -> Result<bool> {
        let m = &ctx.accounts.market;
        require!(m.status == MarketStatus::Settled as u8, VarError::NotSettled);
        let rc = &m.receipt;
        let status = decode_status(rc.status_code)?;
        let pools = Pools::new(m.pool_home, m.pool_draw, m.pool_away);
        let r = rulebook_resolve(
            &MatchState::new(rc.home_goals, rc.away_goals, status),
            pools,
            m.fee_bps,
        );
        let matches = encode_outcome(r.outcome) == rc.outcome_code
            && r.settlement.paid_as_refund == rc.paid_as_refund
            && r.settlement.net == rc.net
            && r.settlement.fee == rc.fee
            && r.settlement.winning_pool == rc.winning_pool;
        Ok(matches)
    }
}

const RESOLVE_GRACE_SECS: i64 = 7 * 24 * 60 * 60; // settlement may land after the deadline (root publish lag)

fn decode_status(code: u8) -> Result<MatchStatus> {
    Ok(match code {
        0 => MatchStatus::Completed,
        1 => MatchStatus::CompletedAfterExtraTime,
        2 => MatchStatus::CompletedAfterPenalties,
        3 => MatchStatus::Abandoned,
        4 => MatchStatus::Postponed,
        5 => MatchStatus::Void,
        _ => return err!(VarError::InvalidStatus),
    })
}

fn encode_outcome(o: Outcome) -> u8 {
    match o {
        Outcome::Home => OUT_HOME,
        Outcome::Draw => OUT_DRAW,
        Outcome::Away => OUT_AWAY,
        Outcome::Refund => OUT_REFUND,
    }
}

// ------------------------------------------------------------------------------------------------
// Tx LINE (`Txoracle`) CPI — raw invoke matching the on-chain IDL (docs/idl/txoracle_mainnet.json).
// ------------------------------------------------------------------------------------------------
mod txoracle_cpi {
    use super::*;
    use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
    use anchor_lang::solana_program::program::{get_return_data, invoke};

    // Anchor discriminator for `validate_stat` (from IDL).
    const VALIDATE_STAT_DISCM: [u8; 8] = [107, 197, 232, 90, 191, 136, 105, 185];

    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct ScoresUpdateStats {
        pub update_count: i32,
        pub min_timestamp: i64,
        pub max_timestamp: i64,
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct ScoresBatchSummary {
        pub fixture_id: i64,
        pub update_stats: ScoresUpdateStats,
        pub events_sub_tree_root: [u8; 32],
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct ProofNode {
        pub hash: [u8; 32],
        pub is_right_sibling: bool,
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct ScoreStat {
        pub key: u32,
        pub value: i32,
        pub period: i32,
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct StatTerm {
        pub stat_to_prove: ScoreStat,
        pub event_stat_root: [u8; 32],
        pub stat_proof: Vec<ProofNode>,
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub enum Comparison {
        GreaterThan,
        LessThan,
        EqualTo,
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct TraderPredicate {
        pub threshold: i32,
        pub comparison: Comparison,
    }
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub enum BinaryExpression {
        Add,
        Subtract,
    }

    // Argument tuple for validate_stat, serialized after the discriminator (order matches IDL).
    #[derive(AnchorSerialize)]
    struct ValidateStatArgs<'a> {
        ts: i64,
        fixture_summary: &'a ScoresBatchSummary,
        fixture_proof: &'a Vec<ProofNode>,
        main_tree_proof: &'a Vec<ProofNode>,
        predicate: TraderPredicate,
        stat_a: &'a StatTerm,
        stat_b: Option<StatTerm>,
        op: Option<BinaryExpression>,
    }

    /// CPI `validate_stat` with an `EqualTo` predicate proving `stat_a.value == threshold` against
    /// the daily root. Returns the program's `bool` result (read from return data).
    pub fn validate_stat_equal<'info>(
        txoracle_program: &AccountInfo<'info>,
        daily_scores_merkle_roots: &AccountInfo<'info>,
        witness: &StatWitness,
        threshold: i32,
    ) -> Result<bool> {
        let mut data = VALIDATE_STAT_DISCM.to_vec();
        ValidateStatArgs {
            ts: witness.ts,
            fixture_summary: &witness.summary,
            fixture_proof: &witness.fixture_proof,
            main_tree_proof: &witness.main_tree_proof,
            predicate: TraderPredicate { threshold, comparison: Comparison::EqualTo },
            stat_a: &witness.stat,
            stat_b: None,
            op: None,
        }
        .serialize(&mut data)?;

        let ix = Instruction {
            program_id: *txoracle_program.key,
            accounts: vec![AccountMeta::new_readonly(*daily_scores_merkle_roots.key, false)],
            data,
        };
        invoke(&ix, &[daily_scores_merkle_roots.clone(), txoracle_program.clone()])?;

        let (_pid, ret) = get_return_data().ok_or(VarError::NoReturnData)?;
        // Borsh bool = 1 byte (0/1).
        Ok(ret.first().copied().unwrap_or(0) == 1)
    }
}

// A witness authenticating one stat: the fixture summary, the two proof paths, and the stat term.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct StatWitness {
    pub ts: i64,
    pub summary: txoracle_cpi::ScoresBatchSummary,
    pub fixture_proof: Vec<txoracle_cpi::ProofNode>,
    pub main_tree_proof: Vec<txoracle_cpi::ProofNode>,
    pub stat: txoracle_cpi::StatTerm,
}

// ------------------------------------------------------------------------------------------------
// State
// ------------------------------------------------------------------------------------------------
#[repr(u8)]
pub enum MarketStatus {
    Open = 0,
    Settled = 1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct ResolutionReceipt {
    pub source_root: [u8; 32],
    pub ruleset_hash: [u8; 32],
    pub home_goals: i32,
    pub away_goals: i32,
    pub status_code: u8,
    pub outcome_code: u8,
    pub paid_as_refund: bool,
    pub pot: u64,
    pub fee: u64,
    pub net: u64,
    pub winning_pool: u64,
    pub resolved_ts: i64,
}

#[account]
pub struct Market {
    pub fixture_id: i64,
    pub market_kind: u8,
    pub home_stat_key: u32,
    pub away_stat_key: u32,
    pub period: i32,
    pub fee_bps: u16,
    pub resolve_deadline: i64,
    pub status: u8,
    pub usdc_mint: Pubkey,
    pub vault: Pubkey,
    pub pool_home: u64,
    pub pool_draw: u64,
    pub pool_away: u64,
    // Two-step resolution: home goals are authenticated first (attest_home) and cached here, because
    // both Merkle proofs together exceed the 1232-byte transaction limit.
    pub pending_home_goals: i32,
    pub pending_home_set: bool,
    pub home_events_root: [u8; 32],
    pub receipt: ResolutionReceipt,
    pub bump: u8,
}

impl Market {
    // discriminator(8) + fixed fields + pending-home cache + receipt
    pub const SPACE: usize =
        8 + 8 + 1 + 4 + 4 + 4 + 2 + 8 + 1 + 32 + 32 + 8 + 8 + 8 + (4 + 1 + 32) + ResolutionReceipt::SPACE + 1;
}
impl ResolutionReceipt {
    pub const SPACE: usize = 32 + 32 + 4 + 4 + 1 + 1 + 1 + 8 + 8 + 8 + 8 + 8;
}

#[account]
pub struct Position {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub stake_home: u64,
    pub stake_draw: u64,
    pub stake_away: u64,
    pub claimed: bool,
    pub bump: u8,
}
impl Position {
    pub const SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1 + 1;
}

// ------------------------------------------------------------------------------------------------
// Instruction args & contexts
// ------------------------------------------------------------------------------------------------
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateMarketArgs {
    pub fixture_id: i64,
    pub market_kind: u8,
    pub home_stat_key: u32,
    pub away_stat_key: u32,
    pub period: i32,
    pub fee_bps: u16,
    pub resolve_deadline: i64,
}

#[derive(Accounts)]
#[instruction(args: CreateMarketArgs)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = Market::SPACE,
        seeds = [b"market", args.fixture_id.to_le_bytes().as_ref(), &[args.market_kind]],
        bump
    )]
    pub market: Account<'info, Market>,
    pub usdc_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(
        init,
        payer = creator,
        seeds = [b"vault", market.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = vault,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(outcome: u8, amount: u64)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,
    #[account(mut, seeds = [b"market", market.fixture_id.to_le_bytes().as_ref(), &[market.market_kind]], bump = market.bump)]
    pub market: Account<'info, Market>,
    #[account(mut, address = market.vault)]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = market.usdc_mint, token::authority = depositor)]
    pub depositor_ata: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = depositor,
        space = Position::SPACE,
        seeds = [b"position", market.key().as_ref(), depositor.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Resolve<'info> {
    /// Anyone may resolve (permissionless).
    pub resolver: Signer<'info>,
    #[account(mut, seeds = [b"market", market.fixture_id.to_le_bytes().as_ref(), &[market.market_kind]], bump = market.bump)]
    pub market: Account<'info, Market>,
    /// The Tx LINE daily scores Merkle roots PDA (seeds ["daily_scores_roots", epoch_day u16 LE]).
    /// CHECK: validated by the Txoracle program during CPI.
    pub daily_scores_merkle_roots: UncheckedAccount<'info>,
    /// CHECK: the Tx LINE `Txoracle` program; verified by address at the call site in production.
    pub txoracle_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub claimant: Signer<'info>,
    #[account(seeds = [b"market", market.fixture_id.to_le_bytes().as_ref(), &[market.market_kind]], bump = market.bump)]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        seeds = [b"position", market.key().as_ref(), claimant.key().as_ref()],
        bump = position.bump,
        has_one = market,
        constraint = position.owner == claimant.key() @ VarError::NotPositionOwner,
    )]
    pub position: Account<'info, Position>,
    #[account(mut, address = market.vault)]
    pub vault: Account<'info, TokenAccount>,
    /// CHECK: PDA vault authority (== vault owner); seeds checked here.
    #[account(seeds = [b"vault", market.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, token::mint = market.usdc_mint, token::authority = claimant)]
    pub recipient_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Reverify<'info> {
    #[account(seeds = [b"market", market.fixture_id.to_le_bytes().as_ref(), &[market.market_kind]], bump = market.bump)]
    pub market: Account<'info, Market>,
}

#[event]
pub struct MarketResolved {
    pub market: Pubkey,
    pub fixture_id: i64,
    pub outcome_code: u8,
    pub paid_as_refund: bool,
    pub home_goals: i32,
    pub away_goals: i32,
}

#[error_code]
pub enum VarError {
    #[msg("Unsupported market kind")]
    UnsupportedMarketKind,
    #[msg("Fee exceeds maximum")]
    FeeTooHigh,
    #[msg("Resolve deadline must be in the future")]
    DeadlineInPast,
    #[msg("Market is not open")]
    MarketNotOpen,
    #[msg("Market is closed for deposits")]
    MarketClosed,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Invalid outcome index")]
    InvalidOutcome,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Market already settled")]
    AlreadySettled,
    #[msg("Resolve window has passed")]
    ResolveWindowPassed,
    #[msg("Witness fixture does not match this market")]
    FixtureMismatch,
    #[msg("Stat key does not match market configuration")]
    StatKeyMismatch,
    #[msg("Stat period does not match market configuration")]
    StatPeriodMismatch,
    #[msg("Stat failed on-chain authentication")]
    StatNotAuthenticated,
    #[msg("Home goals must be attested (attest_home) before resolve")]
    HomeNotAttested,
    #[msg("Txoracle returned no data")]
    NoReturnData,
    #[msg("Invalid match status code")]
    InvalidStatus,
    #[msg("Market not settled")]
    NotSettled,
    #[msg("Position already claimed")]
    AlreadyClaimed,
    #[msg("Signer is not the position owner")]
    NotPositionOwner,
}
