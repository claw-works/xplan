CREATE TYPE client_role AS ENUM ('user', 'admin');
ALTER TABLE client_keys ADD COLUMN role client_role NOT NULL DEFAULT 'user';
