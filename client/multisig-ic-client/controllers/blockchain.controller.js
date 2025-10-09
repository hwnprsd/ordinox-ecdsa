/* eslint-disable no-undef */
/* eslint-disable @typescript-eslint/no-require-imports */
const blockchainService = require('../services/blockchain.service');
const { successResponse } = require('../utils/responses');

class BlockchainController {
  async createOrSignTransaction(req, res, next) {
    try {
      const { chain, toAddress, amount, signatoryName, msgId } = req.body;
      
      // Validate signatoryName is one of the configured signatories
      const availableSignatories = await blockchainService.getSignatoryNames();
      if (!availableSignatories.includes(signatoryName)) {
        return res.status(400).json({
          success: false,
          message: `Invalid signatory. Available signatories: ${availableSignatories.join(', ')}`
        });
      }

      const result = await blockchainService.createOrSignTransaction(
        chain,
        toAddress,
        amount,
        signatoryName,
        msgId
      );
      
      const statusCode = result.executed ? 200 : 201; // 200 for executed, 201 for pending
      return successResponse(res, result, result.message, statusCode);
    } catch (error) {
      next(error);
    }
  }

  async initMultisig(req, res, next) {
    try {
      const { chain } = req.body;
      const result = await blockchainService.initMultisig(chain);
      return successResponse(res, result, 'Multisig initialized successfully');
    } catch (error) {
      next(error);
    }
  }
          
  async getWalletInfo(req, res, next) {
    try {
      const { chain } = req.params;
      console.log(chain)
      const info = await blockchainService.getWalletInfo(chain);
      return successResponse(res, info);
    } catch (error) {
      next(error);
    }
  }

  async getTransactions(req, res, next) {
    try {
      const { status } = req.query;
      const transactions = await blockchainService.getTransactions(status);
      return successResponse(res, { 
        transactions,
        multisigInfo: await blockchainService.getMultisigInfo()
      });
    } catch (error) {
      next(error);
    }
  }

  async getPendingTransactions(req, res, next) {
    try {
      const pendingTransactions = await blockchainService.getPendingTransactions();
      const multisigInfo = await blockchainService.getMultisigInfo();
      
      return successResponse(res, {
        pendingTransactions,
        multisigInfo,
        signatoriesStatus: await blockchainService.getSignatoriesStatus()
      });
    } catch (error) {
      next(error);
    }
  }

  async getMultisigInfo(req, res, next) {
    try {
      const info = await blockchainService.getMultisigInfo();
      const signatories = await blockchainService.getAllSignatories();
      const signatoriesStatus = await blockchainService.getSignatoriesStatus();
      
      return successResponse(res, {
        ...info,
        signatories,
        signatoriesStatus,
        threshold: info.threshold || 2,
        configuredFromService: true
      });
    } catch (error) {
      next(error);
    }
  }

  async getSignatories(req, res, next) {
    try {
      const signatories = await blockchainService.getAllSignatories();
      const signatoriesStatus = await blockchainService.getSignatoriesStatus();
      const availableNames = await blockchainService.getSignatoryNames();
      
      return successResponse(res, {
        signatories,
        signatoriesStatus,
        availableSignatoryNames: availableNames,
        totalSignatories: signatories.length,
        note: 'These are the configured signatories from the service config'
      });
    } catch (error) {
      next(error);
    }
  }

  async verifySignatories(req, res, next) {
    try {
      const verificationResults = await blockchainService.verifyAllSignatories();
      
      const summary = {
        totalSignatories: verificationResults.length,
        connectedCount: verificationResults.filter(r => r.status === 'connected').length,
        failedCount: verificationResults.filter(r => r.status === 'failed').length,
        allConnected: verificationResults.every(r => r.status === 'connected')
      };

      return successResponse(res, {
        summary,
        details: verificationResults,
        note: 'Verification of configured signatories from service config'
      });
    } catch (error) {
      next(error);
    }
  }

  async getTransactionStatus(req, res, next) {
    try {
      const { transactionId } = req.params;
      const status = await blockchainService.getTransactionStatus(transactionId);
      
      return successResponse(res, status);
    } catch (error) {
      next(error);
    }
  }

  async getSupportedChains(req, res, next) {
    try {
      const chains = blockchainService.getSupportedChains();
      
      return successResponse(res, { 
        chains,
        multisigEnabled: true,
        configuredSignatories: await blockchainService.getSignatoryNames()
      });
    } catch (error) {
      next(error);
    }
  }

  async   getWalletBalance(req, res, next) {
    try {
      const balance = await blockchainService.getWalletBalance();
      const multisigInfo = await blockchainService.getMultisigInfo();
      
      return successResponse(res, {
        balance,
        multisigInfo,
        walletAddress: await blockchainService.getWalletAddress()
      });
    } catch (error) {
      next(error);
    }
  }

  async getAvailableSignatories(req, res, next) {
    try {
      const signatoryNames = await blockchainService.getSignatoryNames();
      const signatories = await blockchainService.getAllSignatories();
      
      return successResponse(res, {
        availableSignatories: signatoryNames,
        signatoriesDetails: signatories,
        note: 'Use these signatory names when creating or signing transactions'
      });
    } catch (error) {
      next(error);
    }
  }
}

module.exports = new BlockchainController();
