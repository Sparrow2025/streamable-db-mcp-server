-- MySQL MCP Server Test Database Initialization
-- This script creates sample tables and data for testing the MCP server

USE mcp_test;

-- Create users table
CREATE TABLE IF NOT EXISTS users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    age INT,
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- Create orders table
CREATE TABLE IF NOT EXISTS orders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT,
    product_name VARCHAR(255) NOT NULL,
    quantity INT NOT NULL DEFAULT 1,
    price DECIMAL(10, 2) NOT NULL,
    status ENUM('pending', 'processing', 'shipped', 'delivered', 'cancelled') DEFAULT 'pending',
    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Create products table
CREATE TABLE IF NOT EXISTS products (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    price DECIMAL(10, 2) NOT NULL,
    stock_quantity INT DEFAULT 0,
    category VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert sample users
INSERT INTO users (name, email, age, active) VALUES
('Alice Johnson', 'alice@example.com', 28, TRUE),
('Bob Smith', 'bob@example.com', 35, TRUE),
('Charlie Brown', 'charlie@example.com', 22, FALSE),
('Diana Prince', 'diana@example.com', 30, TRUE),
('Eve Wilson', 'eve@example.com', 27, TRUE);

-- Insert sample products
INSERT INTO products (name, description, price, stock_quantity, category) VALUES
('Laptop Pro', 'High-performance laptop for professionals', 1299.99, 50, 'Electronics'),
('Wireless Mouse', 'Ergonomic wireless mouse with long battery life', 29.99, 200, 'Electronics'),
('Coffee Mug', 'Ceramic coffee mug with company logo', 12.99, 100, 'Office Supplies'),
('Notebook', 'Premium leather-bound notebook', 24.99, 75, 'Office Supplies'),
('Desk Lamp', 'LED desk lamp with adjustable brightness', 89.99, 30, 'Furniture');

-- Insert sample orders
INSERT INTO orders (user_id, product_name, quantity, price, status) VALUES
(1, 'Laptop Pro', 1, 1299.99, 'delivered'),
(1, 'Wireless Mouse', 2, 29.99, 'delivered'),
(2, 'Coffee Mug', 3, 12.99, 'shipped'),
(3, 'Notebook', 1, 24.99, 'cancelled'),
(4, 'Desk Lamp', 1, 89.99, 'processing'),
(5, 'Laptop Pro', 1, 1299.99, 'pending'),
(2, 'Wireless Mouse', 1, 29.99, 'delivered');

-- Create a view for order summaries
CREATE VIEW order_summary AS
SELECT 
    u.name as customer_name,
    u.email as customer_email,
    o.id as order_id,
    o.product_name,
    o.quantity,
    o.price,
    o.status,
    o.order_date
FROM orders o
JOIN users u ON o.user_id = u.id;

-- Create an index for better performance
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_status ON orders(status);

-- Grant necessary permissions to the MCP user
GRANT SELECT ON mcp_test.* TO 'mcp_user'@'%';
FLUSH PRIVILEGES;