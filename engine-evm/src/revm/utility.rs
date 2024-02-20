use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage::{address_to_key, bytes_to_key, storage_to_key, KeyPrefix};
use aurora_engine_types::types::{u256_to_arr, Address};
use aurora_engine_types::{Vec, H160, H256, U256};

const BLOCK_HASH_PREFIX: u8 = 0;
const BLOCK_HASH_PREFIX_SIZE: usize = 1;
const BLOCK_HEIGHT_SIZE: usize = 8;
const CHAIN_ID_SIZE: usize = 32;

/// Get contract storage by index
pub fn get_storage<I: IO>(
    io: &I,
    address: &revm::primitives::Address,
    key: &revm::primitives::U256,
    generation: u32,
) -> revm::primitives::U256 {
    let raw_key = key.to_be_bytes();
    let key = H256::from(raw_key);
    let raw = io
        .read_storage(storage_to_key(&from_address(address), &key, generation).as_ref())
        .and_then(|value| {
            if value.len() == 32 {
                let mut buf = [0u8; 32];
                value.copy_to_slice(&mut buf);
                Some(H256(buf))
            } else {
                None
            }
        })
        .unwrap_or_default();
    revm::primitives::U256::from_be_slice(raw.as_ref())
}

/// Get EVM code from contract storage
pub fn get_code<I: IO>(io: &I, address: &revm::primitives::Address) -> Vec<u8> {
    io.read_storage(&address_to_key(KeyPrefix::Code, &from_address(address)))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

/// Get EVM code by `code_hash` from contract storage
pub fn get_code_by_code_hash<I: IO>(io: &I, code_hash: &revm::primitives::B256) -> Vec<u8> {
    io.read_storage(&storage_key(code_hash.0.as_slice()))
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

/// Convert REVM `Address` to Engine `Address`
fn from_address(address: &revm::primitives::Address) -> Address {
    Address::new(H160::from(address.0 .0))
}

/// Get balance from contract storage
pub fn get_balance<I: IO>(io: &I, address: &revm::primitives::Address) -> revm::primitives::U256 {
    let addr = from_address(address);
    let mut raw: Vec<u8> = Vec::new();
    io.read_u256(&address_to_key(KeyPrefix::Balance, &addr))
        .unwrap_or_else(|_| U256::zero())
        .to_big_endian(&mut raw);
    revm::primitives::U256::from_be_slice(&raw)
}

/// Get nonce from contract storage
pub fn get_nonce<I: IO>(io: &I, address: &revm::primitives::Address) -> u64 {
    io.read_u256(&address_to_key(KeyPrefix::Nonce, &from_address(address)))
        .unwrap_or_else(|_| U256::zero())
        .as_u64()
}

pub fn get_generation<I: IO>(io: &I, address: &revm::primitives::Address) -> u32 {
    io.read_storage(&address_to_key(
        KeyPrefix::Generation,
        &from_address(address),
    ))
    .map_or(0, |value| {
        let mut bytes = [0u8; 4];
        value.copy_to_slice(&mut bytes);
        u32::from_be_bytes(bytes)
    })
}

/// There is one Aurora block per NEAR block height (note: when heights in NEAR are skipped
/// they are interpreted as empty blocks on Aurora). The blockhash is derived from the height
/// according to
/// ```text
/// block_hash = sha256(concat(
///     BLOCK_HASH_PREFIX,
///     block_height as u64,
///     chain_id,
///     engine_account_id,
/// ))
/// ```
#[must_use]
pub fn compute_block_hash(
    chain_id: [u8; 32],
    block_height: revm::primitives::U256,
    account_id: &[u8],
) -> revm::primitives::B256 {
    debug_assert_eq!(
        BLOCK_HASH_PREFIX_SIZE,
        core::mem::size_of_val(&BLOCK_HASH_PREFIX)
    );
    debug_assert_eq!(CHAIN_ID_SIZE, core::mem::size_of_val(&chain_id));
    let mut data = Vec::with_capacity(
        BLOCK_HASH_PREFIX_SIZE + BLOCK_HEIGHT_SIZE + CHAIN_ID_SIZE + account_id.len(),
    );
    let height = block_height.to_be_bytes_vec();
    data.push(BLOCK_HASH_PREFIX);
    data.extend_from_slice(&chain_id);
    data.extend_from_slice(account_id);
    data.extend_from_slice(&height);

    let hash = aurora_engine_sdk::sha256(&data).0;
    revm::primitives::B256::new(hash)
}

/// Contract storage key for `CodeHash`
fn storage_key(value: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(1 + value.len());
    bytes.extend_from_slice(value);
    bytes_to_key(KeyPrefix::CodeHash, &bytes)
}

pub fn set_balance<I: IO>(
    io: &mut I,
    address: &revm::primitives::Address,
    balance: &revm::primitives::U256,
) {
    let balance = balance.to_be_bytes_vec();
    io.write_storage(
        &address_to_key(KeyPrefix::Balance, &from_address(address)),
        &balance,
    );
}

pub fn set_nonce<I: IO>(io: &mut I, address: &revm::primitives::Address, nonce: u64) {
    let nonce = U256::from(nonce);
    io.write_storage(
        &address_to_key(KeyPrefix::Nonce, &from_address(address)),
        &u256_to_arr(&nonce),
    );
}

pub fn set_code<I: IO>(io: &mut I, address: &revm::primitives::Address, code: &[u8]) {
    io.write_storage(
        &address_to_key(KeyPrefix::Code, &from_address(address)),
        code,
    );
}

/// Removes an account.
pub fn remove_account<I: IO + Copy>(
    io: &mut I,
    address: &revm::primitives::Address,
    generation: u32,
) {
    remove_nonce(io, address);
    remove_balance(io, address);
    remove_code(io, address);
    remove_all_storage(io, address, generation);
}

fn remove_nonce<I: IO>(io: &mut I, address: &revm::primitives::Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Nonce, &from_address(address)));
}

pub fn remove_balance<I: IO + Copy>(io: &mut I, address: &revm::primitives::Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Balance, &from_address(address)));
}

pub fn remove_code<I: IO>(io: &mut I, address: &revm::primitives::Address) {
    io.remove_storage(&address_to_key(KeyPrefix::Code, &from_address(address)));
}

/// Removes all storage for the given address.
pub fn remove_all_storage<I: IO>(io: &mut I, address: &revm::primitives::Address, generation: u32) {
    // FIXME: there is presently no way to prefix delete trie state.
    // NOTE: There is not going to be a method on runtime for this.
    //     You may need to store all keys in a list if you want to do this in a contract.
    //     Maybe you can incentivize people to delete dead old keys. They can observe them from
    //     external indexer node and then issue special cleaning transaction.
    //     Either way you may have to store the nonce per storage address root. When the account
    //     has to be deleted the storage nonce needs to be increased, and the old nonce keys
    //     can be deleted over time. That's how TurboGeth does storage.
    set_generation(io, address, generation + 1);
}

/// Increments storage generation for a given address.
pub fn set_generation<I: IO>(io: &mut I, address: &revm::primitives::Address, generation: u32) {
    io.write_storage(
        &address_to_key(KeyPrefix::Generation, &from_address(address)),
        &generation.to_be_bytes(),
    );
}

pub fn remove_storage<I: IO>(
    io: &mut I,
    address: &revm::primitives::Address,
    key: &H256,
    generation: u32,
) {
    io.remove_storage(storage_to_key(&from_address(address), key, generation).as_ref());
}

pub fn set_storage<I: IO>(
    io: &mut I,
    address: &revm::primitives::Address,
    key: &H256,
    value: &H256,
    generation: u32,
) {
    io.write_storage(
        storage_to_key(&from_address(address), key, generation).as_ref(),
        &value.0,
    );
}
