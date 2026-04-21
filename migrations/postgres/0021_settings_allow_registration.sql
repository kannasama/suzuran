INSERT INTO settings (key, value) VALUES ('allow_registration', 'true')
ON CONFLICT (key) DO NOTHING;
