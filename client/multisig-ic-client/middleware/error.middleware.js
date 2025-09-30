/* eslint-disable @typescript-eslint/no-require-imports */
/* eslint-disable no-undef */
const logger = require('../utils/logger');

const errorHandler = (err, req, res, next) => {
  // Log the error stack trace
  logger.error(err.stack || err.toString());

  // Check if response headers have already been sent
  if (res.headersSent) {
    return next(err);
  }

  // Set status code (default to 500 if not provided)
  const status = err.status || 500;
  const message = err.message || 'Internal server error';

  // Send error response in JSON format
  res.status(status).json({
    success: false,
    error: message,
    ...(process.env.NODE_ENV === 'development' && { stack: err.stack })
  });
};

module.exports = errorHandler;
