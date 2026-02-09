// Application configuration
// WARNING: This file contains intentionally planted secrets for testing

export const config = {
    port: 3000,
    host: "localhost",

    // SEC Error: AWS Access Key ID (matches AKIA[0-9A-Z]{16})
    awsAccessKeyId: "AKIAIOSFODNN7EXAMPLE",

    // SEC Error: GitHub token (matches gh[pousr]_[A-Za-z0-9_]{36,})
    githubToken: "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn",

    // SEC Warning: Generic API key (matches api[_-]?key\s*[:=]\s*['"][a-zA-Z0-9]{20,}['"])
    api_key: "abcdefghijklmnopqrstuvwxyz1234567890ABCD",

    jwtSecret: "change-me-in-production",
};
