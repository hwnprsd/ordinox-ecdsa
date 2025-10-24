/* eslint-disable no-undef */
/* eslint-disable @typescript-eslint/no-require-imports */
const EC = require('elliptic').ec;
const { Actor, HttpAgent } = require('@dfinity/agent');
const { Secp256k1KeyIdentity } = require('@dfinity/identity-secp256k1');
const { Principal } = require('@dfinity/principal');
const fs = require('fs');
const config = require('../config');
const logger = require('../utils/logger');

class ICService {
  constructor() {
    this.agents = new Map();
    this.identities = new Map();
    this.actors = new Map();
    this.signatories = [];
    this.threshold = config.ic.threshold || 2;
    this.initialized = false;
    this.supportedChains = ['solana', 'xrp', 'evm','cardano'];
  }

  async initialize() {
    // If already initialized, return immediately
    if (this.initialized) return;
    
    // If initialization is in progress, wait for it to complete
    if (this.initializing && this.initPromise) {
      return await this.initPromise;
    }

    // Start initialization
    this.initializing = true;
    this.initPromise = this._performInitialization();
    
    try {
      await this.initPromise;
    } finally {
      this.initializing = false;
    }
  }

  async _performInitialization() {
    try {
      // Validate config has signatories
      if (!config.ic.signatories || !Array.isArray(config.ic.signatories)) {
        throw new Error('config.ic.signatories must be defined as an array');
      }

      // Load all signatory identities from config
      for (const sigConfig of config.ic.signatories) {
        console.log(sigConfig)
        await this.loadSignatory(sigConfig.name, sigConfig.identityPath);
      }

      // Verify we have enough signatories
      if (this.signatories.length < this.threshold) {
        throw new Error(`Not enough signatories loaded. Need at least ${this.threshold}, got ${this.signatories.length}`);
      }

      this.initialized = true;
      logger.info(`IC Service initialized with ${this.signatories.length} signatories, threshold: ${this.threshold}`);
      logger.info('Signatory principals:', this.signatories.map(s => s.principal));

    } catch (error) {
      logger.error('Failed to initialize IC Service:', error);
      // Reset state on failure
      this.initialized = false;
      this.initPromise = null;
      throw error;
    }
  }

  // Helper method to ensure initialization before method execution
  async ensureInitialized() {
    if (!this.initialized) {
      await this.initialize();
    }
  }

  async loadSignatory(name, identityPath) {
    try {
      // Validate inputs first
      if (!name) {
        throw new Error('Signatory name is required');
      }
      
      if (!identityPath) {
        throw new Error(`Identity path is required for signatory '${name}'. Check your configuration.`);
      }

      // Check if file exists
      if (!fs.existsSync(identityPath)) {
        throw new Error(`Identity file not found for signatory '${name}' at path: ${identityPath}`);
      }

      // Read PEM, remove header/footer, join lines
      const pem = fs.readFileSync(identityPath, 'utf8');
      const lines = pem
        .split('\n')
        .filter(line => !line.includes('-----'))
        .join('');
      const der = Buffer.from(lines, 'base64');

      // Parse ASN.1 using elliptic
      const ec = new EC('secp256k1');
      const key = ec.keyFromPrivate(der.slice(-32));

      // Get the raw 32-byte private key
      const privBuf = key.getPrivate().toArrayLike(Buffer, 'be', 32);

      // Load into ICP identity
      const identity = Secp256k1KeyIdentity.fromSecretKey(privBuf);
      const principal = identity.getPrincipal().toText();

      // Store identity and create agent
      this.identities.set(name, identity);
      
      const agent = new HttpAgent({
        host: config.ic.host,
        identity: identity
      });

      agent.defaultStrategy = require('@dfinity/agent').polling.strategy.defaultStrategy;
      agent.pollingStrategy = require('@dfinity/agent').polling.strategy.defaultStrategy({
          timeout: 600000, // 10 minutes
          maxAttempts: 120  // More attempts with longer timeout
      });
      
      if (config.env === 'development') {
        await agent.fetchRootKey();
      }

      this.agents.set(name, agent);

      // Create actor for this signatory
      const actor = Actor.createActor(this.getIDL(), {
        agent: agent,
        canisterId: Principal.fromText(config.ic.canisterIds.ordinox)
      });

      this.actors.set(name, actor);
      //setting the canister ids
      await actor.set_canister_ids(config.ic.canisterIds.solRpc,config.ic.canisterIds.basicSolana);
      await actor.get_eth_key_info();
      // Add to signatories list
      this.signatories.push({
        name,
        identity,
        principal,
        agent,
        actor
      });

      logger.info(`Loaded signatory ${name}: ${principal}`);

    } catch (error) {
      logger.error(`Failed to load signatory ${name}:`, {
        identityPath,
        error: error.message
      });
      throw new Error(`Failed to load signatory '${name}': ${error.message}`);
    }
  }

  getIDL() {
    return ({ IDL }) => {
      const Result = IDL.Variant({
        Ok: IDL.Text,
        Err: IDL.Text
      });

      // Multi-chain configuration
      const ChainConfig = IDL.Record({
        chain_id: IDL.Text,    // "evm", "solana", "xrp"
        signers: IDL.Vec(IDL.Principal),
        threshold: IDL.Nat32,
        network: IDL.Text      // "mainnet", "testnet", "devnet"
      });

      // Legacy transaction record (for backward compatibility)
      const TransactionRecord = IDL.Record({
        id: IDL.Text,
        chain: IDL.Text,
        signers: IDL.Vec(IDL.Principal),
        to_address: IDL.Text,
        amount: IDL.Text,
        executed: IDL.Bool,
        tx_hash: IDL.Opt(IDL.Text),
        timestamp: IDL.Nat64
      });

      const EthKeyInfo = IDL.Record({
        public_key: IDL.Vec(IDL.Nat8),
        compressed_key: IDL.Vec(IDL.Nat8),
        eth_address: IDL.Text,
        derivation_path: IDL.Vec(IDL.Vec(IDL.Nat8))
      });

    const EthKeyInfoResult = IDL.Variant({
      Ok: EthKeyInfo,  // EthKeyInfo is already defined in your IDL
      Err: IDL.Text
    });
      return IDL.Service({
        // === NEW UNIFIED MULTI-CHAIN INTERFACE ===
        
        // Multi-chain initialization
        init_multisig: IDL.Func(
          [IDL.Vec(ChainConfig)],
          [Result],
          []
        ),
        init_all_chains: IDL.Func(
          [IDL.Vec(IDL.Principal), IDL.Nat32],
          [Result],
          []
        ),
        
        // Unified transaction interface
        create_or_sign_transaction: IDL.Func(
          [IDL.Text, IDL.Text, IDL.Text, IDL.Text], // [chain, msg_id, to_address, amount]
          [Result],
          []
        ),
        
        // Chain utilities
        get_eth_key_info: IDL.Func([], [EthKeyInfoResult], []),
        get_supported_chains: IDL.Func([], [IDL.Vec(IDL.Text)], ['query']),
        get_wallet_address: IDL.Func([IDL.Text], [Result], []), // [chain]
        get_wallet_balance: IDL.Func([IDL.Text], [Result], []), // [chain]
        
        // Health check
        health_check: IDL.Func([], [IDL.Text], ['query']),

        //Solana
        create_or_sign_solana_transaction: IDL.Func(
          [IDL.Text, IDL.Text, IDL.Text], // [msg_id, to_address, amount]
          [Result],
          []
        ),
        get_canister_solana_address: IDL.Func([], [Result], []),
        get_pending_transactions: IDL.Func(
          [],
          [IDL.Vec(IDL.Tuple(IDL.Text, TransactionRecord))],
          ['query']
        ),
        get_signers_and_threshold: IDL.Func(
          [],
          [IDL.Vec(IDL.Principal), IDL.Nat32],
          ['query']
        ),
        set_canister_ids: IDL.Func([IDL.Text, IDL.Text], [Result], [])
      });
    };
  }

  // === NEW UNIFIED MULTI-CHAIN METHODS ===

  // Initialize specific chains with individual configurations
  async initializeMultisig(chainConfigs) {
    await this.ensureInitialized();
    
    if (!Array.isArray(chainConfigs)) {
      throw new Error('chainConfigs must be an array');
    }

    const primaryActor = this.actors.values().next().value;
    const configs = chainConfigs.map(chain => ({
      chain_id: chain,
      signers: this.signatories.map(s => Principal.fromText(s.principal)),
      threshold: this.threshold,
      network: 'mainnet'
    }));

    logger.info(`Initializing multisig for chains:`, configs.map(c => `${c.chain_id} (${c.network})`));
    return await primaryActor.init_multisig(configs);
  }

  // Initialize all supported chains with same configuration
  async initializeAllChains() {
    await this.ensureInitialized();
    
    const primaryActor = this.actors.values().next().value;
    const signerPrincipals = this.signatories.map(s => Principal.fromText(s.principal));
    const threshold = Number(this.threshold);

    if (!Number.isInteger(threshold) || threshold < 0 || threshold > 4294967295) {
      throw new Error(`Invalid threshold for Nat32: ${this.threshold}`);
    }

    logger.info(`Initializing all chains with ${signerPrincipals.length} signers, threshold: ${threshold}`);
    return await primaryActor.init_all_chains(signerPrincipals, threshold);
  }

  // Unified transaction creation method
  async createOrSignTransaction(chain, msgId, signatoryName, toAddress, amount) {
    await this.ensureInitialized();
    
    // Validate chain
    if (!this.supportedChains.includes(chain)) {
      throw new Error(`Unsupported chain: ${chain}. Supported chains: ${this.supportedChains.join(', ')}`);
    }

    const actor = this.actors.get(signatoryName);
    if (!actor) {
      throw new Error(`Signatory ${signatoryName} not found`);
    }

    const signatoryPrincipal = this.signatories.find(s => s.name === signatoryName)?.principal;
    logger.info(`${signatoryName} (${signatoryPrincipal}) creating/signing ${chain.toUpperCase()} transaction to ${toAddress} for ${amount}`);
    
    return await actor.create_or_sign_transaction(chain, msgId, toAddress, amount);
  }

  // Get supported chains
  async getSupportedChains() {
    await this.ensureInitialized();
    const actor = this.actors.values().next().value;
    return await actor.get_supported_chains();
  }

  // Get wallet address for specific chain
  async getWalletAddress(chain, signatoryName = null) {
    await this.ensureInitialized();
    
    if (!this.supportedChains.includes(chain)) {
      throw new Error(`Unsupported chain: ${chain}`);
    }

    const actor = signatoryName ? this.actors.get(signatoryName) : this.actors.values().next().value;
    return await actor.get_wallet_address(chain);
  }

  // Get wallet balance for specific chain
  async getWalletBalance(chain, signatoryName = null) {
    await this.ensureInitialized();
    
    if (!this.supportedChains.includes(chain)) {
      throw new Error(`Unsupported chain: ${chain}`);
    }

    const actor = signatoryName ? this.actors.get(signatoryName) : this.actors.values().next().value;
    return await actor.get_wallet_balance(chain);
  }

  // Health check
  async healthCheck() {
    await this.ensureInitialized();
    const actor = this.actors.values().next().value;
    return await actor.health_check();
  }

  // === CHAIN-SPECIFIC CONVENIENCE METHODS ===

  // Solana convenience methods
  async createSolanaTransaction(msgId, signatoryName, toAddress, amount) {
    return await this.createOrSignTransaction('solana', msgId, signatoryName, toAddress, amount);
  }

  async getSolanaAddress(signatoryName = null) {
    return await this.getWalletAddress('solana', signatoryName);
  }

  async getSolanaBalance(signatoryName = null) {
    return await this.getWalletBalance('solana', signatoryName);
  }

  // Ethereum convenience methods
  async createEthereumTransaction(msgId, signatoryName, toAddress, amount) {
    return await this.createOrSignTransaction('evm', msgId, signatoryName, toAddress, amount);
  }

  async getEthereumAddress(signatoryName = null) {
    return await this.getWalletAddress('evm', signatoryName);
  }

  async getEthereumBalance(signatoryName = null) {
    return await this.getWalletBalance('evm', signatoryName);
  }


  // XRP convenience methods
  async createXrpTransaction(msgId, signatoryName, toAddress, amount) {
    return await this.createOrSignTransaction('xrp', msgId, signatoryName, toAddress, amount);
  }

  async getXrpAddress(signatoryName = null) {
    return await this.getWalletAddress('xrp', signatoryName);
  }

  async getXrpBalance(signatoryName = null) {
    return await this.getWalletBalance('xrp', signatoryName);
  }

  // === LEGACY METHODS (for backward compatibility) ===

  // Legacy Solana methods - these now call the unified interface
  async createOrSignSolanaTransaction(msgId, signatoryName, toAddress, amount) {
    return await this.createSolanaTransaction(msgId, signatoryName, toAddress, amount);
  }

  async getCanisterSolanaAddress() {
    return await this.getSolanaAddress();
  }

  // Legacy multisig initialization
  async initializeMultisigLegacy(chain, network = "mainnet") {
    const chainConfigs = [{
      chainId: chain,
      network: network,
      signers: this.signatories.map(s => Principal.fromText(s.principal)),
      threshold: this.threshold
    }];
    
    return await this.initializeMultisig(chainConfigs);
  }

  // === UTILITY METHODS ===

  getPrincipal(signatoryName = null) {
    if (signatoryName) {
      const signatory = this.signatories.find(s => s.name === signatoryName);
      return signatory?.principal;
    }
    return this.signatories[0]?.principal;
  }

  getAllPrincipals() {
    return this.signatories.map(s => ({
      name: s.name,
      principal: s.principal
    }));
  }

  getSignatoryNames() {
    return this.signatories.map(s => s.name);
  }

  getSupportedChainsSync() {
    return [...this.supportedChains];
  }

   async setCanisterIds() {
    await this.ensureInitialized();
    const actor = this.actors.values().next().value;
    return await actor.set_canister_ids(config.ic.canisterIds.solRpc,config.ic.canisterIds.basicSolana);
 }


  async verifyAllSignatories() {
    await this.ensureInitialized();
    
    const verifications = await Promise.allSettled(
      this.signatories.map(async (signatory) => {
        try {
          await signatory.agent.status();
          return {
            name: signatory.name,
            principal: signatory.principal,
            status: 'connected'
          };
        } catch (error) {
          return {
            name: signatory.name,
            principal: signatory.principal,
            status: 'failed',
            error: error.message
          };
        }
      })
    );

    return verifications.map(result => result.value || result.reason);
  }

  // Multi-chain transaction helper
  async createTransactionForChains(transactions) {
    await this.ensureInitialized();
    
    const results = [];
    for (const tx of transactions) {
      try {
        const result = await this.createOrSignTransaction(
          tx.chain,
          tx.msgId,
          tx.signatoryName,
          tx.toAddress,
          tx.amount
        );
        results.push({
          chain: tx.chain,
          msgId: tx.msgId,
          success: true,
          result: result
        });
      } catch (error) {
        results.push({
          chain: tx.chain,
          msgId: tx.msgId,
          success: false,
          error: error.message
        });
      }
    }
    
    return results;
  }
}

module.exports = new ICService();