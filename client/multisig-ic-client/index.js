/* eslint-disable no-undef */
/* eslint-disable @typescript-eslint/no-require-imports */
require('dotenv').config();
const app = require('./app');
const config = require('./config');
const logger = require('./utils/logger');

const PORT = config.port;

app.listen(PORT, () => {
  logger.info(`Server running on port ${PORT} in ${config.env} mode`);
  logger.info(`Multisig enabled with ${config.ic.signatories.length} signatories, threshold: ${config.ic.threshold}`);
});

process.on('unhandledRejection', (err) => {
  logger.error('Unhandled rejection:', err);
  process.exit(1);
});

process.on('SIGTERM', () => {
  logger.info('SIGTERM received, shutting down gracefully');
  process.exit(0);
});
