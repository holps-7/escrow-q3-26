use {
    anchor_lang::{
        prelude::{msg, Clock},
        solana_program::instruction::Instruction,
        solana_program::program_pack::Pack,
        system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
        token::spl_token,
    },
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::Keypair,
    solana_message::Message,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (LiteSVM, Keypair) {
    let program_id = escrowq32026::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Return the LiteSVM instance and payer keypair
    (svm, payer)
}

#[test]
fn test_escrow_lifecycle() {
    // Setup the test environment by initializing LiteSVM and creating a payer keypair
    let (mut program, payer) = setup();

    // Get the maker's public key from the payer keypair
    let maker = payer.pubkey();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_a = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint A: {}\n", mint_a);

    let mint_b = CreateMint::new(&mut program, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint B: {}\n", mint_b);

    // Create the maker's associated token account for Mint A
    // This is done using litesvm-token's CreateAssociatedTokenAccount utility
    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();
    msg!("Maker ATA A: {}\n", maker_ata_a);

    // Derive the PDA for the escrow account using the maker's public key and a seed value
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;
    msg!("Escrow PDA: {}\n", escrow);

    // Derive the PDA for the vault associated token account using the escrow PDA and Mint A
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    msg!("Vault PDA: {}\n", vault);

    // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
    MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    // Reusable account metas for every "Make" attempt (same maker/mints/PDAs)
    let make_accounts = escrowq32026::accounts::Make {
        maker: maker,
        mint_a: mint_a,
        mint_b: mint_b,
        maker_ata_a: maker_ata_a,
        escrow: escrow,
        vault: vault,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        token_program: TOKEN_PROGRAM_ID,
        system_program: SYSTEM_PROGRAM_ID,
    }
    .to_account_metas(None);

    // -------------------test make with zero deposit (should fail)-------------------
    let now = program.get_sysvar::<Clock>().unix_timestamp;
    let expiration = now + 1_000;

    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: make_accounts.clone(),
        data: escrowq32026::instruction::Make {
            seed: 123u64,
            deposit: 0,
            receive: 10_000_000,
            expiration: expiration,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    let zero_deposit_result = program.send_transaction(transaction);
    msg!("\n\nMake with zero deposit should fail");
    if let Err(e) = &zero_deposit_result {
        msg!("Zero deposit error: {:?}", e.err);
    }
    assert!(zero_deposit_result.is_err());
    // Whole transaction rolled back — no escrow created
    assert!(program.get_account(&escrow).is_none());

    // -------------------test make with past expiration (should fail)-------------------
    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: make_accounts.clone(),
        data: escrowq32026::instruction::Make {
            seed: 123u64,
            deposit: 10_000_000,
            receive: 10_000_000,
            expiration: now - 1,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    let past_expiration_result = program.send_transaction(transaction);
    msg!("\n\nMake with past expiration should fail");
    if let Err(e) = &past_expiration_result {
        msg!("Past expiration error: {:?}", e.err);
    }
    assert!(past_expiration_result.is_err());

    // -------------------test make-------------------
    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: make_accounts.clone(),
        data: escrowq32026::instruction::Make {
            seed: 123u64,
            deposit: 10_000_000,
            receive: 10_000_000,
            expiration: expiration,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message.clone(), recent_blockhash);

    let tx = program.send_transaction(transaction).unwrap();
    msg!("\n\nMake transaction successful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Verify the vault account and escrow account data after the "Make" instruction
    let vault_account = program.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10_000_000);
    assert_eq!(vault_data.owner, escrow);
    assert_eq!(vault_data.mint, mint_a);

    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.seed, 123u64);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.receive, 10_000_000);
    assert_eq!(escrow_data.expiration, expiration);

    // -------------------test re-make (should fail, escrow already exists)-------------------
    program.expire_blockhash();
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message.clone(), recent_blockhash);

    let remake_result = program.send_transaction(transaction);
    msg!("\n\nRe-make should fail (escrow already exists)");
    if let Err(e) = &remake_result {
        msg!("Re-make error: {:?}", e.err);
    }
    assert!(remake_result.is_err());

    // -------------------test update (extend expiration + raise price)-------------------
    let update_accounts = escrowq32026::accounts::Update {
        maker: maker,
        escrow: escrow,
    }
    .to_account_metas(None);

    let new_expiration = expiration + 1_000;
    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: update_accounts.clone(),
        data: escrowq32026::instruction::Update {
            receive: Some(20_000_000),
            expiration: new_expiration,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    let tx = program.send_transaction(transaction).unwrap();
    msg!("\n\nUpdate transaction successful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.receive, 20_000_000);
    assert_eq!(escrow_data.expiration, new_expiration);

    // -------------------test update shortening expiration (should fail)-------------------
    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: update_accounts.clone(),
        data: escrowq32026::instruction::Update {
            receive: None,
            expiration: expiration,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    let shorten_result = program.send_transaction(transaction);
    msg!("\n\nUpdate shortening the expiration should fail");
    if let Err(e) = &shorten_result {
        msg!("Shorten expiration error: {:?}", e.err);
    }
    assert!(shorten_result.is_err());

    // -------------------test update by non-maker (should fail)-------------------
    let attacker = Keypair::new();
    program.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Update {
            maker: attacker.pubkey(),
            escrow: escrow,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Update {
            receive: Some(1),
            expiration: new_expiration + 1_000,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&attacker.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&attacker], message, recent_blockhash);

    let attacker_update_result = program.send_transaction(transaction);
    msg!("\n\nUpdate by non-maker should fail");
    if let Err(e) = &attacker_update_result {
        msg!("Non-maker update error: {:?}", e.err);
    }
    assert!(attacker_update_result.is_err());

    // -------------------test take-------------------
    let taker_kp = Keypair::new();
    program.airdrop(&taker_kp.pubkey(), 1_000_000_000).unwrap();
    let taker = taker_kp.pubkey();

    // Fund the taker with token B so they can pay the receive amount
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_b)
        .owner(&taker)
        .send()
        .unwrap();
    MintTo::new(&mut program, &payer, &mint_b, &taker_ata_b, 1_000_000_000)
        .send()
        .unwrap();

    // These two are created by the program via init_if_needed
    let taker_ata_a = associated_token::get_associated_token_address(&taker, &mint_a);
    let maker_ata_b = associated_token::get_associated_token_address(&maker, &mint_b);

    let take_accounts = escrowq32026::accounts::Take {
        taker: taker,
        maker: maker,
        mint_a: mint_a,
        mint_b: mint_b,
        maker_ata_b: maker_ata_b,
        taker_ata_a: taker_ata_a,
        taker_ata_b: taker_ata_b,
        escrow: escrow,
        vault: vault,
        associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
        token_program: TOKEN_PROGRAM_ID,
        system_program: SYSTEM_PROGRAM_ID,
    }
    .to_account_metas(None);

    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: take_accounts.clone(),
        data: escrowq32026::instruction::Take {}.data(),
    };

    let message = Message::new(&[ix], Some(&taker));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&taker_kp], message, recent_blockhash);

    let tx = program.send_transaction(transaction).unwrap();
    msg!("\n\nTake transaction successful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Taker received token A, maker received token B at the updated price
    let taker_ata_a_data =
        spl_token::state::Account::unpack(&program.get_account(&taker_ata_a).unwrap().data)
            .unwrap();
    assert_eq!(taker_ata_a_data.amount, 10_000_000);

    let maker_ata_b_data =
        spl_token::state::Account::unpack(&program.get_account(&maker_ata_b).unwrap().data)
            .unwrap();
    assert_eq!(maker_ata_b_data.amount, 20_000_000);

    // Escrow and vault are closed
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());

    // -------------------test make for the refund flow-------------------
    let expiration3 = program.get_sysvar::<Clock>().unix_timestamp + 100;
    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: make_accounts.clone(),
        data: escrowq32026::instruction::Make {
            seed: 123u64,
            deposit: 10_000_000,
            receive: 10_000_000,
            expiration: expiration3,
        }
        .data(),
    };

    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    let tx = program.send_transaction(transaction).unwrap();
    msg!("\n\nMake for refund flow successful");
    msg!("Tx Signature: {}", tx.signature);

    // -------------------test take after expiration (should fail)-------------------
    // Warp the clock sysvar past the expiration
    let mut clock = program.get_sysvar::<Clock>();
    clock.unix_timestamp = expiration3 + 1;
    program.set_sysvar::<Clock>(&clock);

    // Same ix bytes as the previous take — expire the blockhash to get a fresh tx signature
    program.expire_blockhash();
    let ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: take_accounts.clone(),
        data: escrowq32026::instruction::Take {}.data(),
    };

    let message = Message::new(&[ix], Some(&taker));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&taker_kp], message, recent_blockhash);

    let expired_take_result = program.send_transaction(transaction);
    msg!("\n\nTake after expiration should fail");
    if let Err(e) = &expired_take_result {
        msg!("Expired take error: {:?}", e.err);
    }
    assert!(expired_take_result.is_err());

    // Vault still holds the deposit
    let vault_data =
        spl_token::state::Account::unpack(&program.get_account(&vault).unwrap().data).unwrap();
    assert_eq!(vault_data.amount, 10_000_000);

    // -------------------test refund-------------------
    let refund_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Refund {
            maker: maker,
            mint_a: mint_a,
            maker_ata_a: maker_ata_a,
            escrow: escrow,
            vault: vault,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Refund {}.data(),
    };

    let message = Message::new(&[refund_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    let tx = program.send_transaction(transaction).unwrap();
    msg!("\n\nRefund transaction successful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Deposit returned: 1000 - 10 (make 1) - 10 (make 2) + 10 refunded = 990 tokens
    let maker_ata_a_data =
        spl_token::state::Account::unpack(&program.get_account(&maker_ata_a).unwrap().data)
            .unwrap();
    assert_eq!(maker_ata_a_data.amount, 990_000_000);

    // Escrow and vault are closed
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());
}
