function requiredUrl(envName) {
  const value = process.env[envName];
  return value.replace(/\/$/, "");
}

module.exports = { requiredUrl };
