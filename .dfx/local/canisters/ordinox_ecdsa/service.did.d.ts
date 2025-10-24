import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface BalanceInfo {
  'account_id' : string,
  'icp_e8s' : bigint,
  'cycles' : bigint,
}
export interface ChainConfig {
  'threshold' : number,
  'signers' : Array<Principal>,
  'network' : string,
  'chain_id' : string,
}
export interface ConfigInfo {
  'threshold' : number,
  'signers' : Array<Principal>,
  'network' : Network,
  'address' : [] | [string],
  'ecdsa_key' : string,
}
export interface EthKeyInfo {
  'public_key' : Uint8Array | number[],
  'derivation_path' : Array<Uint8Array | number[]>,
  'compressed_key' : Uint8Array | number[],
  'eth_address' : string,
}
export interface EthSignature {
  'r' : Uint8Array | number[],
  's' : Uint8Array | number[],
  'v' : bigint,
}
export interface EthTransaction {
  'to' : string,
  'signature' : [] | [EthSignature],
  'value' : string,
  'data' : Uint8Array | number[],
  'from' : string,
  'chain_id' : bigint,
  'nonce' : bigint,
  'gas_limit' : bigint,
  'gas_price' : string,
}
export interface EthTransactionRecord {
  'id' : string,
  'signers' : Array<Principal>,
  'transaction' : EthTransaction,
  'timestamp' : bigint,
  'executed' : boolean,
  'tx_hash' : [] | [string],
}
export interface HttpHeader { 'value' : string, 'name' : string }
export interface HttpResponse {
  'status' : bigint,
  'body' : Uint8Array | number[],
  'headers' : Array<HttpHeader>,
}
export type Network = { 'Preprod' : null } |
  { 'Mainnet' : null };
export type Result = { 'Ok' : BalanceInfo } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : string } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : TransactionRecord } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : EthKeyInfo } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : XrpKeyInfo } |
  { 'Err' : string };
export interface TransactionRecord {
  'tx_id' : [] | [string],
  'lamports' : bigint,
  'signers' : Array<Principal>,
  'to_address' : string,
  'executed' : boolean,
}
export interface TransformArgs {
  'context' : Uint8Array | number[],
  'response' : HttpResponse,
}
export interface XrpKeyInfo {
  'xrp_address' : string,
  'public_key' : Uint8Array | number[],
  'derivation_path' : Array<Uint8Array | number[]>,
  'compressed_key' : Uint8Array | number[],
}
export interface XrpTransaction {
  'fee' : string,
  'flags' : number,
  'destination' : string,
  'txn_signature' : [] | [string],
  'signing_pub_key' : string,
  'destination_tag' : [] | [number],
  'account' : string,
  'amount' : string,
  'sequence' : number,
}
export interface XrpTransactionRecord {
  'id' : string,
  'signers' : Array<Principal>,
  'transaction' : XrpTransaction,
  'timestamp' : bigint,
  'executed' : boolean,
  'tx_hash' : [] | [string],
}
export interface _SERVICE {
  'check_ledger_balance' : ActorMethod<[string], Result>,
  'create_or_sign_cardano_transaction' : ActorMethod<
    [string, string, string],
    Result_1
  >,
  'create_or_sign_eth_transaction_dynamic_gas' : ActorMethod<
    [string, string, string],
    Result_1
  >,
  'create_or_sign_transaction' : ActorMethod<
    [string, string, string, string],
    Result_1
  >,
  'create_or_sign_xrp_transaction' : ActorMethod<
    [string, string, string],
    Result_1
  >,
  'get_all_transactions' : ActorMethod<[], Array<[string, TransactionRecord]>>,
  'get_balance' : ActorMethod<[], Result_1>,
  'get_cardano_address' : ActorMethod<[], Result_1>,
  'get_cardano_transaction' : ActorMethod<[string], Result_2>,
  'get_config' : ActorMethod<[], ConfigInfo>,
  'get_cycles' : ActorMethod<[], bigint>,
  'get_eth_address' : ActorMethod<[], Result_1>,
  'get_eth_balance' : ActorMethod<[], Result_1>,
  'get_eth_key_info' : ActorMethod<[], Result_3>,
  'get_eth_signers_and_threshold' : ActorMethod<[], [Array<Principal>, number]>,
  'get_eth_transactions' : ActorMethod<
    [],
    Array<[string, EthTransactionRecord]>
  >,
  'get_executed_transactions' : ActorMethod<
    [],
    Array<[string, TransactionRecord]>
  >,
  'get_pending_transactions' : ActorMethod<
    [],
    Array<[string, TransactionRecord]>
  >,
  'get_signers_and_threshold' : ActorMethod<[], [Array<Principal>, number]>,
  'get_supported_chains' : ActorMethod<[], Array<string>>,
  'get_transaction' : ActorMethod<[string], [] | [TransactionRecord]>,
  'get_transaction_signers' : ActorMethod<[string], Array<Principal>>,
  'get_wallet_address' : ActorMethod<[string], Result_1>,
  'get_wallet_balance' : ActorMethod<[string], Result_1>,
  'get_xrp_address' : ActorMethod<[], Result_1>,
  'get_xrp_balance' : ActorMethod<[], Result_1>,
  'get_xrp_key_info' : ActorMethod<[], Result_4>,
  'get_xrp_signers_and_threshold' : ActorMethod<[], [Array<Principal>, number]>,
  'get_xrp_transactions' : ActorMethod<
    [],
    Array<[string, XrpTransactionRecord]>
  >,
  'health_check' : ActorMethod<[], string>,
  'init_all_chains' : ActorMethod<[Array<Principal>, number], Result_1>,
  'init_eth_multisig' : ActorMethod<[Array<Principal>, number], Result_1>,
  'init_multisig' : ActorMethod<[Array<ChainConfig>], Result_1>,
  'init_xrp_multisig' : ActorMethod<[Array<Principal>, number], Result_1>,
  'request_airdrop' : ActorMethod<[], Result_1>,
  'set_canister_ids' : ActorMethod<[string, string], Result_1>,
  'set_ecdsa_key_name' : ActorMethod<[string], Result_1>,
  'transform' : ActorMethod<[TransformArgs], HttpResponse>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
