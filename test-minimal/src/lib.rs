use near_sdk::{env, near_bindgen, AccountId, BorshStorageKey};
use near_sdk::collections::UnorderedMap;
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey)]
pub enum StorageKey {
    Test,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub enum TestEnum {
    A,
    B,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct TestStruct {
    pub field1: AccountId,
    pub field2: String,
    pub field3: TestEnum,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct TestContract {
    data: UnorderedMap<AccountId, TestStruct>,
}

#[near_bindgen]
impl TestContract {
    pub fn test_method(&self) -> u32 {
        self.data.len() as u32
    }
}
