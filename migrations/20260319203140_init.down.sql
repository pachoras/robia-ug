-- Add down migration script here
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS user_profiles;
DROP TABLE IF EXISTS user_auth_tokens;
DROP TABLE IF EXISTS registration_tokens;
DROP TABLE IF EXISTS password_reset_tokens;
DROP TABLE IF EXISTS additional_files;