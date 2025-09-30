/* eslint-disable @typescript-eslint/no-require-imports */
/* eslint-disable no-undef */
require('dotenv').config();

module.exports = {
  port: process.env.PORT || 3000,
  env: process.env.NODE_ENV || 'development',
  cors: {
    origin: process.env.CORS_ORIGIN || '*',
    credentials: true
  },
  ic: {
    host: process.env.IC_HOST,
    threshold: parseInt(process.env.IC_THRESHOLD) || 2, // 2 out of 3 signatures required
    canisterIds: {
      ordinox: process.env.ORDINOX_CANISTER_ID,
      basicSolana: process.env.BASIC_SOLANA_CANISTER_ID,
      solRpc: process.env.SOL_RPC_CANISTER_ID
    },
    // Legacy single identity (for backward compatibility)
    identityPath: process.env.IDENTITY_PATH || './identity.pem',
    
    // Multiple signatories for multisig
    signatories: [
      {
        name: 'mainnet',
        identityPath: process.env.SIGNATORY_1_PATH 
      },
      {
        name: 'mainnet2', 
        identityPath: process.env.SIGNATORY_2_PATH 
      }
    ]
  },
  chains: {
    solana: {
      network: process.env.SOLANA_NETWORK || 'devnet'
    },
    evm: {
      network: process.env.EVM_NETWORK || 'mainnet'
    },
    xrp: {
      network: process.env.XRP_NETWORK || 'testnet'
    }
  }
};