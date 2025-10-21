export const idlFactory = ({ IDL }) => {
  const BalanceInfo = IDL.Record({
    'account_id' : IDL.Text,
    'icp_e8s' : IDL.Nat64,
    'cycles' : IDL.Nat,
  });
  const Result = IDL.Variant({ 'Ok' : BalanceInfo, 'Err' : IDL.Text });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text });
  const EthKeyInfo = IDL.Record({
    'public_key' : IDL.Vec(IDL.Nat8),
    'derivation_path' : IDL.Vec(IDL.Vec(IDL.Nat8)),
    'compressed_key' : IDL.Vec(IDL.Nat8),
    'eth_address' : IDL.Text,
  });
  const Result_2 = IDL.Variant({ 'Ok' : EthKeyInfo, 'Err' : IDL.Text });
  const EthSignature = IDL.Record({
    'r' : IDL.Vec(IDL.Nat8),
    's' : IDL.Vec(IDL.Nat8),
    'v' : IDL.Nat64,
  });
  const EthTransaction = IDL.Record({
    'to' : IDL.Text,
    'signature' : IDL.Opt(EthSignature),
    'value' : IDL.Text,
    'data' : IDL.Vec(IDL.Nat8),
    'from' : IDL.Text,
    'chain_id' : IDL.Nat64,
    'nonce' : IDL.Nat64,
    'gas_limit' : IDL.Nat64,
    'gas_price' : IDL.Text,
  });
  const EthTransactionRecord = IDL.Record({
    'id' : IDL.Text,
    'signers' : IDL.Vec(IDL.Principal),
    'transaction' : EthTransaction,
    'timestamp' : IDL.Nat64,
    'executed' : IDL.Bool,
    'tx_hash' : IDL.Opt(IDL.Text),
  });
  const TransactionRecord = IDL.Record({
    'tx_id' : IDL.Opt(IDL.Text),
    'lamports' : IDL.Nat64,
    'signers' : IDL.Vec(IDL.Principal),
    'to_address' : IDL.Text,
    'executed' : IDL.Bool,
  });
  const XrpKeyInfo = IDL.Record({
    'xrp_address' : IDL.Text,
    'public_key' : IDL.Vec(IDL.Nat8),
    'derivation_path' : IDL.Vec(IDL.Vec(IDL.Nat8)),
    'compressed_key' : IDL.Vec(IDL.Nat8),
  });
  const Result_3 = IDL.Variant({ 'Ok' : XrpKeyInfo, 'Err' : IDL.Text });
  const XrpTransaction = IDL.Record({
    'fee' : IDL.Text,
    'flags' : IDL.Nat32,
    'destination' : IDL.Text,
    'txn_signature' : IDL.Opt(IDL.Text),
    'signing_pub_key' : IDL.Text,
    'destination_tag' : IDL.Opt(IDL.Nat32),
    'account' : IDL.Text,
    'amount' : IDL.Text,
    'sequence' : IDL.Nat32,
  });
  const XrpTransactionRecord = IDL.Record({
    'id' : IDL.Text,
    'signers' : IDL.Vec(IDL.Principal),
    'transaction' : XrpTransaction,
    'timestamp' : IDL.Nat64,
    'executed' : IDL.Bool,
    'tx_hash' : IDL.Opt(IDL.Text),
  });
  const ChainConfig = IDL.Record({
    'threshold' : IDL.Nat32,
    'signers' : IDL.Vec(IDL.Principal),
    'network' : IDL.Text,
    'chain_id' : IDL.Text,
  });
  const HttpHeader = IDL.Record({ 'value' : IDL.Text, 'name' : IDL.Text });
  const HttpResponse = IDL.Record({
    'status' : IDL.Nat,
    'body' : IDL.Vec(IDL.Nat8),
    'headers' : IDL.Vec(HttpHeader),
  });
  const TransformArgs = IDL.Record({
    'context' : IDL.Vec(IDL.Nat8),
    'response' : HttpResponse,
  });
  return IDL.Service({
    'check_ledger_balance' : IDL.Func([], [Result], []),
    'create_or_sign_eth_transaction_dynamic_gas' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result_1],
        [],
      ),
    'create_or_sign_transaction' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Text],
        [Result_1],
        [],
      ),
    'create_or_sign_xrp_transaction' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result_1],
        [],
      ),
    'get_cycles' : IDL.Func([], [IDL.Nat], ['query']),
    'get_eth_address' : IDL.Func([], [Result_1], ['query']),
    'get_eth_balance' : IDL.Func([], [Result_1], []),
    'get_eth_key_info' : IDL.Func([], [Result_2], []),
    'get_eth_signers_and_threshold' : IDL.Func(
        [],
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        ['query'],
      ),
    'get_eth_transactions' : IDL.Func(
        [],
        [IDL.Vec(IDL.Tuple(IDL.Text, EthTransactionRecord))],
        ['query'],
      ),
    'get_executed_transactions' : IDL.Func(
        [],
        [IDL.Vec(IDL.Tuple(IDL.Text, TransactionRecord))],
        ['query'],
      ),
    'get_pending_transactions' : IDL.Func(
        [],
        [IDL.Vec(IDL.Tuple(IDL.Text, TransactionRecord))],
        ['query'],
      ),
    'get_signers_and_threshold' : IDL.Func(
        [],
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        ['query'],
      ),
    'get_supported_chains' : IDL.Func([], [IDL.Vec(IDL.Text)], ['query']),
    'get_transaction' : IDL.Func(
        [IDL.Text],
        [IDL.Opt(TransactionRecord)],
        ['query'],
      ),
    'get_transaction_signers' : IDL.Func(
        [IDL.Text],
        [IDL.Vec(IDL.Principal)],
        ['query'],
      ),
    'get_wallet_address' : IDL.Func([IDL.Text], [Result_1], []),
    'get_wallet_balance' : IDL.Func([IDL.Text], [Result_1], []),
    'get_xrp_address' : IDL.Func([], [Result_1], ['query']),
    'get_xrp_balance' : IDL.Func([], [Result_1], []),
    'get_xrp_key_info' : IDL.Func([], [Result_3], []),
    'get_xrp_signers_and_threshold' : IDL.Func(
        [],
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        ['query'],
      ),
    'get_xrp_transactions' : IDL.Func(
        [],
        [IDL.Vec(IDL.Tuple(IDL.Text, XrpTransactionRecord))],
        ['query'],
      ),
    'health_check' : IDL.Func([], [IDL.Text], ['query']),
    'init_all_chains' : IDL.Func(
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        [Result_1],
        [],
      ),
    'init_eth_multisig' : IDL.Func(
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        [Result_1],
        [],
      ),
    'init_multisig' : IDL.Func([IDL.Vec(ChainConfig)], [Result_1], []),
    'init_xrp_multisig' : IDL.Func(
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        [Result_1],
        [],
      ),
    'request_airdrop' : IDL.Func([], [Result_1], []),
    'set_canister_ids' : IDL.Func([IDL.Text, IDL.Text], [Result_1], []),
    'set_ecdsa_key_name' : IDL.Func([IDL.Text], [Result_1], []),
    'transform' : IDL.Func([TransformArgs], [HttpResponse], ['query']),
  });
};
export const init = ({ IDL }) => { return []; };
