/* eslint-disable @typescript-eslint/no-require-imports */
/* eslint-disable no-undef */
const icService = require('./ic.service');
const solanaService = require('./chain-specific/solana.service');
const xrpService = require('./chain-specific/xrp.service');
const logger = require('../utils/logger');

class BlockchainService {
  constructor() {
    this.chainServices = {
      solana: solanaService,
      xrp: xrpService
    };
  }

  // Single unified method for creating/signing transactions
// Single unified method for creating/signing transactions
  async createOrSignTransaction(chain, toAddress, amount, signatoryName, msgId = '') {
    // const chainService = this.chainServices[chain];
    // if (!chainService) {
    //   throw new Error(`Unsupported chain: ${chain}`);
    // }

    // if (!chainService.validateAddress(toAddress)) {
    //   throw new Error(`Invalid ${chain} address format`);
    // }

    // if (chain !== 'solana') {
    //   throw new Error('Only Solana transactions are currently supported');
    // }

    if (!signatoryName) {
      throw new Error('Signatory name is required');
    }

    // Use provided msgId if not empty, otherwise pass empty string to let canister generate
    const transactionMsgId = msgId && msgId.trim() !== '' ? msgId.trim() : '';

    const result = await icService.createOrSignTransaction(
      chain,
      transactionMsgId,
      signatoryName,
      toAddress,
      amount
    );

    if (result.Err) {
      throw new Error(result.Err);
    }

    // Parse response to extract transaction info
    let transactionId = transactionMsgId || 'auto-generated';
    let signaturesCount = 0;
    let executed = false;

    if (result.Ok) {
      // Extract transaction ID from response if we didn't provide one
      if (!transactionMsgId) {
        const idMatch = result.Ok.match(/Transaction (\w+)/);
        if (idMatch) {
          transactionId = idMatch[1];
        }
      }

      // Extract signature count (e.g., "1/3 signatures" or "2/3 signatures")
      const sigMatch = result.Ok.match(/(\d+)\/(\d+) signatures/);
      if (sigMatch) {
        signaturesCount = parseInt(sigMatch[1]);
      }

      // Check if executed
      executed = result.Ok.toLowerCase().includes('executed');
    }

    return {
      success: true,
      transactionId,
      signedBy: signatoryName,
      chain,
      toAddress,
      amount,
      message: result.Ok,
      executed,
      signaturesCount,
      threshold: icService.threshold
    };
  }

  async initMultisig(chain) {
    try {
      const result = await icService.initializeMultisig(chain);

      if (result.Err) {
        throw new Error(result.Err);
      }

      const signatories = icService.getAllPrincipals();

      return {
        success: true,
        message: result.Ok,
        chain,
        signatories: signatories.length,
        threshold: icService.threshold,
        signatoryPrincipals: signatories
      };
    } catch (error) {
      logger.error('Init multisig failed:', error);
      throw error;
    }
  }

  async getWalletInfo(chain) {
    try{
  
      const addressResult = await icService.getWalletAddress(chain);
      const balanceResult = await icService.getWalletBalance(chain);

      if (addressResult.Err) throw new Error(addressResult.Err);
      if (balanceResult.Err) throw new Error(balanceResult.Err);


      return {
        chain: chain,
        address: addressResult.Ok,
        balance: balanceResult.Ok,
        multisigEnabled: true
      };
    } catch (error) {
      logger.error('Solana getWalletInfo failed:', error);
      throw error;
    }
  }

  async getTransactions(status = 'pending') {
    if (status === 'pending') {
      const transactions = await icService.getPendingTransactions();
      
      return transactions.map(([id, tx]) => ({
        id,
        chain: tx.chain,
        toAddress: tx.to_address,
        amount: tx.amount,
        signers: tx.signers.map(p => p.toText()),
        executed: tx.executed,
        txHash: tx.tx_hash?.[0] || null,
        timestamp: Number(tx.timestamp),
        status: tx.executed ? 'executed' : 'pending'
      }));
    } else {
      const allTransactions = await this.getTransactions('pending');
      return allTransactions.filter(tx => tx.executed);
    }
  }

  async getPendingTransactions() {
    return this.getTransactions('pending');
  }

  async getTransactionStatus(transactionId) {
    const pendingTxs = await this.getPendingTransactions();
    const tx = pendingTxs.find(t => t.id === transactionId);
    
    if (!tx) {
      throw new Error(`Transaction ${transactionId} not found`);
    }

    return {
      transactionId,
      status: tx.executed ? 'executed' : 'pending',
      chain: tx.chain,
      toAddress: tx.toAddress,
      amount: tx.amount,
      signers: tx.signers,
      txHash: tx.txHash,
      timestamp: tx.timestamp,
      signaturesRequired: icService.threshold,
      signaturesCollected: tx.signers.length
    };
  }


  async getAllSignatories() {
    return icService.getAllPrincipals();
  }

  

  async getSignatoriesStatus() {
    return icService.verifyAllSignatories();
  }

  async getSignatoryNames() {
    return icService.getSignatoryNames();
  }

  async verifyAllSignatories() {
    return icService.verifyAllSignatories();
  }

  async getWalletBalance(chain) {
    return icService.getWalletBalance(chain);
  }

  async getWalletAddress(chain) {
     return icService.getWalletAddress(chain);

  }

  getSupportedChains() {
    return Object.keys(this.chainServices);
  }
}

module.exports = new BlockchainService();
