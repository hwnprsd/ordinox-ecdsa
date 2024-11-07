import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface Message {
  'id' : bigint,
  'signature' : [] | [string],
  'signers' : Array<Principal>,
  'data' : string,
}
export interface State {
  'threshold' : number,
  'messages' : Array<Message>,
  'signers' : Array<Principal>,
  'next_id' : bigint,
}
export interface _SERVICE {
  'caller' : ActorMethod<[], Principal>,
  'create_or_sign_message' : ActorMethod<
    [string],
    { 'Ok' : bigint } |
      { 'Err' : string }
  >,
  'get_signature' : ActorMethod<[string], string>,
  'setup' : ActorMethod<
    [Array<Principal>, number],
    { 'Ok' : null } |
      { 'Err' : string }
  >,
  'state' : ActorMethod<[], State>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
