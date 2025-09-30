/* eslint-disable no-undef */
/* eslint-disable @typescript-eslint/no-require-imports */
const icService = require('../ic.service');
const logger = require('../../utils/logger');

class SolanaService {
  async getWalletInfo() {
    try {
      const addressResult = await icService.getCanisterSolanaAddress();
      const balanceResult = await icService.getWalletBalance();

      if (addressResult.Err) throw new Error(addressResult.Err);
      if (balanceResult.Err) throw new Error(balanceResult.Err);

      const lamports = Number(balanceResult.Ok);
      const sol = lamports / 1e9;

      return {
        chain: 'solana',
        address: addressResult.Ok,
        balance: sol.toFixed(9),
        lamports: lamports.toString(),
        network: process.env.SOLANA_NETWORK || 'devnet',
        multisigEnabled: true
      };
    } catch (error) {
      logger.error('Solana getWalletInfo failed:', error);
      throw error;
    }
  }

  validateAddress(address) {
    const base58Regex = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
    return base58Regex.test(address);
  }

  async estimateFee() {
    return {
      fee: '0.000005',
      currency: 'SOL',
      lamports: '5000'
    };
  }
}

module.exports = new SolanaService();
