export const idlFactory = ({ IDL }) => {
  const Message = IDL.Record({
    'id' : IDL.Nat64,
    'signature' : IDL.Opt(IDL.Text),
    'signers' : IDL.Vec(IDL.Principal),
    'data' : IDL.Text,
  });
  const State = IDL.Record({
    'threshold' : IDL.Nat32,
    'messages' : IDL.Vec(Message),
    'signers' : IDL.Vec(IDL.Principal),
    'next_id' : IDL.Nat64,
  });
  return IDL.Service({
    'caller' : IDL.Func([], [IDL.Principal], ['query']),
    'create_or_sign_message' : IDL.Func(
        [IDL.Text],
        [IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text })],
        [],
      ),
    'get_signature' : IDL.Func([IDL.Text], [IDL.Text], ['query']),
    'setup' : IDL.Func(
        [IDL.Vec(IDL.Principal), IDL.Nat32],
        [IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text })],
        [],
      ),
    'state' : IDL.Func([], [State], ['query']),
  });
};
export const init = ({ IDL }) => { return []; };
