/* eslint-disable no-undef */
/* eslint-disable @typescript-eslint/no-require-imports */
const router = require('express').Router();
const blockchainController = require('../controllers/blockchain.controller');
const validate = require('../middleware/validation.middleware');
const Joi = require('joi');

const schemas = {
  createOrSignTransaction: Joi.object({
    chain: Joi.string().required(),
    toAddress: Joi.string().required(),
    amount: Joi.string().pattern(/^\d+(\.\d+)?$/).required(),
    signatoryName: Joi.string().required(),
    msgId: Joi.string().optional()
  }),
  initMultisig: Joi.object({
    chain: Joi.array().required()
  })
};

// Basic routes
router.get('/chains', blockchainController.getSupportedChains);
router.get('/wallet/:chain', blockchainController.getWalletInfo);
// router.get('/wallet/:chain/balance', blockchainController.getWalletBalance);

// Transaction routes
// router.get('/transactions', blockchainController.getTransactions);
// router.get('/transactions/pending', blockchainController.getPendingTransactions);
// router.get('/transactions/:transactionId/status', blockchainController.getTransactionStatus);

// Multisig routes
// router.get('/multisig', blockchainController.getMultisigInfo);
// router.get('/signatories', blockchainController.getSignatories);
// router.get('/signatories/available', blockchainController.getAvailableSignatories);
// router.get('/signatories/verify', blockchainController.verifySignatories);

// Main transaction endpoint (unified create/sign)
router.post(
  '/transaction',
  validate(schemas.createOrSignTransaction),
  blockchainController.createOrSignTransaction
);

router.post(
  '/multisig/init',
  validate(schemas.initMultisig),
  blockchainController.initMultisig
);

module.exports = router;
