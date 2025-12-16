-- MySQL MCP Server - Development Environment Initialization
-- This script creates sample tables and data for the development environment

USE dev_database;

-- Create sample tables
CREATE TABLE users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE orders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    total_amount DECIMAL(10,2) NOT NULL,
    status ENUM('pending', 'processing', 'completed', 'cancelled') DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE products (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    price DECIMAL(10,2) NOT NULL,
    stock_quantity INT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert sample data
INSERT INTO users (username, email) VALUES
('dev_user1', 'dev1@example.com'),
('dev_user2', 'dev2@example.com'),
('dev_user3', 'dev3@example.com'),
('test_user', 'test@example.com');

INSERT INTO products (name, description, price, stock_quantity) VALUES
('Development Widget', 'A widget for development testing', 19.99, 100),
('Test Product', 'Product for testing purposes', 29.99, 50),
('Sample Item', 'Sample item for demos', 9.99, 200);

INSERT INTO orders (user_id, total_amount, status) VALUES
(1, 19.99, 'completed'),
(2, 29.99, 'processing'),
(1, 39.98, 'pending'),
(3, 9.99, 'completed');

-- Create a view for testing
CREATE VIEW user_order_summary AS
SELECT 
    u.username,
    u.email,
    COUNT(o.id) as order_count,
    COALESCE(SUM(o.total_amount), 0) as total_spent
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.username, u.email;

-- Grant permissions
GRANT SELECT, INSERT, UPDATE, DELETE ON dev_database.* TO 'dev_user'@'%';
FLUSH PRIVILEGES;