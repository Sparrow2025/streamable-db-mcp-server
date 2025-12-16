-- MySQL MCP Server - Staging Environment Initialization
-- This script creates sample tables and data for the staging environment

USE staging_database;

-- Create sample tables (similar to dev but with staging-specific data)
CREATE TABLE users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    is_verified BOOLEAN DEFAULT FALSE
);

CREATE TABLE orders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    total_amount DECIMAL(10,2) NOT NULL,
    status ENUM('pending', 'processing', 'completed', 'cancelled') DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    shipped_at TIMESTAMP NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE products (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    price DECIMAL(10,2) NOT NULL,
    stock_quantity INT DEFAULT 0,
    category VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert staging-specific sample data
INSERT INTO users (username, email, is_verified) VALUES
('staging_user1', 'staging1@example.com', TRUE),
('staging_user2', 'staging2@example.com', TRUE),
('staging_user3', 'staging3@example.com', FALSE),
('qa_tester', 'qa@example.com', TRUE),
('staging_admin', 'admin@staging.example.com', TRUE);

INSERT INTO products (name, description, price, stock_quantity, category) VALUES
('Staging Widget Pro', 'Advanced widget for staging tests', 49.99, 75, 'widgets'),
('QA Test Product', 'Product specifically for QA testing', 99.99, 25, 'test'),
('Staging Sample', 'Sample item for staging environment', 14.99, 150, 'samples'),
('Beta Feature Item', 'Item with beta features', 199.99, 10, 'beta');

INSERT INTO orders (user_id, total_amount, status, shipped_at) VALUES
(1, 49.99, 'completed', '2024-01-15 10:30:00'),
(2, 99.99, 'completed', '2024-01-16 14:20:00'),
(1, 64.98, 'processing', NULL),
(3, 14.99, 'pending', NULL),
(4, 249.98, 'completed', '2024-01-17 09:15:00');

-- Create staging-specific views
CREATE VIEW staging_metrics AS
SELECT 
    COUNT(DISTINCT u.id) as total_users,
    COUNT(DISTINCT CASE WHEN u.is_verified THEN u.id END) as verified_users,
    COUNT(o.id) as total_orders,
    SUM(CASE WHEN o.status = 'completed' THEN o.total_amount ELSE 0 END) as completed_revenue
FROM users u
LEFT JOIN orders o ON u.id = o.user_id;

-- Grant permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON staging_database.* TO 'staging_user'@'%';
FLUSH PRIVILEGES;