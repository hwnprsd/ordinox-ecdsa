/* eslint-disable no-undef */
const successResponse = (res, rawData, message = 'Success', statusCode = 200) => {
   const data = JSON.parse(JSON.stringify(rawData, (_key, value) =>
    typeof value === 'bigint' ? value.toString() : value
  ));
  return res.status(statusCode).json({
    success: true,
    message,
    data
  });
};

const errorResponse = (res, message, statusCode = 400) => {
  return res.status(statusCode).json({
    success: false,
    error: message
  });
};

module.exports = { successResponse, errorResponse };
