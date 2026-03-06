use litesvm::LiteSVM;
use litesvm_token::{spl_token::{self}, CreateMint};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_clock::Clock;

pub const PROGRAM_ID: &str = "4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT";

pub fn program_id() -> Pubkey {
    Pubkey::from(crate::ID)
}

pub struct InitSetup {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub mint: Pubkey,
    pub fundraiser_pda: Pubkey,
    pub bump: u8,
}

pub fn load_svm() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();

    svm
        .airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("airdrop failed");

    let so_path = "/Users/mobius/Desktop/anchor-fundraiser/target/sbpf-solana-solana/release/fundraiser.so";
    // print!("{}", so_path.display()); 
    let program_data = std::fs::read(so_path).expect("read program so");
    let program_id = program_id();

    svm.add_program(program_id, &program_data)
        .expect("add program");

    assert_eq!(program_id.to_string(), PROGRAM_ID);

    (svm, payer)
}

pub fn setup_init(decimals: u8) -> InitSetup {
    let (mut svm, payer) = load_svm();

    // Set a non-zero clock so time_started is meaningful in tests
    set_clock(&mut svm, 1);

    let mint = CreateMint::new(&mut svm, &payer)
        .decimals(decimals)
        .authority(&payer.pubkey())
        .send()
        .expect("create mint");

    let (fundraiser_pda, bump) =
        Pubkey::find_program_address(&[b"fundraiser", payer.pubkey().as_ref()], &program_id());

    InitSetup {
        svm,
        payer,
        mint,
        fundraiser_pda,
        bump,
    }
}

pub fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar::<Clock>(&clock);
}

pub fn build_initialize_ix(
    maker: &Keypair,
    mint_to_raise: &Pubkey,
    fundraiser_pda: &Pubkey,
    bump: u8,
    amount: u64,
    duration: u8,
) -> Instruction {
    let data = [
        vec![0u8],
        amount.to_le_bytes().to_vec(),
        vec![duration],
        vec![bump],
    ]
    .concat();

    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new(*mint_to_raise, false),
            AccountMeta::new(*fundraiser_pda, false),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
            AccountMeta::new(spl_token::ID, false),
        ],
        data,
    }
}

pub fn build_contribute_ix(
    contributor: &Keypair,
    mint_to_raise: &Pubkey,
    fundraiser_pda: &Pubkey,
    contributor_pda: &Pubkey,
    contributor_bump: u8,
    contributor_ata: &Pubkey,
    vault: &Pubkey,
    amount: u64,
) -> Instruction {
    let data = [vec![2u8], amount.to_le_bytes().to_vec(), vec![contributor_bump]].concat();

    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(contributor.pubkey(), true),
            AccountMeta::new_readonly(*mint_to_raise, false),
            AccountMeta::new(*fundraiser_pda, false),
            AccountMeta::new(*contributor_pda, false),
            AccountMeta::new_readonly(*contributor_ata, false),
            AccountMeta::new_readonly(*vault, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        data,
    }
}

pub fn build_check_ix(
    maker: &Keypair,
    mint_to_raise: &Pubkey,
    fundraiser_pda: &Pubkey,
    maker_ata: &Pubkey,
    vault: &Pubkey,
) -> Instruction {
    let data = vec![1u8];

    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new_readonly(*mint_to_raise, false),
            AccountMeta::new(*fundraiser_pda, false),
            AccountMeta::new(*maker_ata, false),
            AccountMeta::new(*vault, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        data,
    }
}

pub fn build_refund_ix(
    contributor: &Keypair,
    maker: &Pubkey,
    mint_to_raise: &Pubkey,
    fundraiser_pda: &Pubkey,
    contributor_pda: &Pubkey,
    contributor_ata: &Pubkey,
    vault: &Pubkey,
) -> Instruction {
    let data = vec![3u8];

    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(contributor.pubkey(), true),
            AccountMeta::new_readonly(*maker, false),
            AccountMeta::new_readonly(*mint_to_raise, false),
            AccountMeta::new(*fundraiser_pda, false),
            AccountMeta::new(*contributor_pda, false),
            AccountMeta::new(*contributor_ata, false),
            AccountMeta::new(*vault, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        data,
    }
}

pub fn send_ix(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) -> u64 {
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let blockhash = svm.latest_blockhash();
    svm
        .send_transaction(Transaction::new(&[payer], msg, blockhash))
        .expect("send tx")
        .compute_units_consumed
}
