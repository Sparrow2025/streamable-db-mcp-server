-- MySQL MCP Server - Production Test Environment Initialization
-- This script creates sample tables and data for production-like testing

USE prod_database;

-- Create production-like tables with additional constraints and indexes
CREATE TABLE users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    is_verified BOOLEAN DEFAULT FALSE,
    last_login TIMESTAMP NULL,
    status ENUM('active', 'inactive', 'suspended') DEFAULT 'active',
    INDEX idx_email (email),
    INDEX idx_status (status),
    INDEX idx_created_at (created_at)
);

CREATE TABLE orders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    order_number VARCHAR(20) NOT NULL UNIQUE,
    total_amount DECIMAL(10,2) NOT NULL,
    status ENUM('pending', 'processing', 'completed', 'cancelled', 'refunded') DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    shipped_at TIMESTAMP NULL,
    delivered_at TIMESTAMP NULL,
    FOREIGN KEY (user_id) REFERENCES users(id),
    INDEX idx_user_id (user_id),
    INDEX idx_status (status),
    INDEX idx_created_at (created_at),
    INDEX idx_order_number (order_number)
);

CREATE TABLE products (
    id INT AUTO_INCREMENT PRIMARY KEY,
    sku VARCHAR(50) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    price DECIMAL(10,2) NOT NULL,
    stock_quantity INT DEFAULT 0,
    category VARCHAR(50),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_sku (sku),
    INDEX idx_category (category),
    INDEX idx_is_active (is_active)
);

CREATE TABLE order_items (
    id INT AUTO_INCREMENT PRIMARY KEY,
    order_id INT NOT NULL,
    product_id INT NOT NULL,
    quantity INT NOT NULL,
    unit_price DECIMAL(10,2) NOT NULL,
    FOREIGN KEY (order_id) REFERENCES orders(id),
    FOREIGN KEY (product_id) REFERENCES products(id),
    INDEX idx_order_id (order_id),
    INDEX idx_product_id (product_id)
);

-- Insert production-like sample data
INSERT INTO users (username, email, is_verified, last_login, status) VALUES
('prod_customer1', 'customer1@company.com', TRUE, '2024-01-20 15:30:00', 'active'),
('prod_customer2', 'customer2@company.com', TRUE, '2024-01-19 09:45:00', 'active'),
('prod_customer3', 'customer3@company.com', TRUE, '2024-01-18 14:20:00', 'active'),
('enterprise_user', 'enterprise@bigcorp.com', TRUE, '2024-01-20 11:15:00', 'active'),
('vip_customer', 'vip@premium.com', TRUE, '2024-01-20 16:45:00', 'active');

INSERT INTO products (sku, name, description, price, stock_quantity, category, is_active) VALUES
('PROD-001', 'Enterprise Widget', 'High-performance widget for enterprise use', 299.99, 50, 'enterprise', TRUE),
('PROD-002', 'Professional Tool', 'Professional-grade tool', 149.99, 100, 'tools', TRUE),
('PROD-003', 'Premium Service', 'Premium service package', 499.99, 25, 'services', TRUE),
('PROD-004', 'Standard Widget', 'Standard widget for general use', 99.99, 200, 'widgets', TRUE),
('PROD-005', 'Legacy Product', 'Legacy product (discontinued)', 79.99, 0, 'legacy', FALSE);

INSERT INTO orders (user_id, order_number, total_amount, status, shipped_at, delivered_at) VALUES
(1, 'ORD-2024-001', 299.99, 'completed', '2024-01-15 10:00:00', '2024-01-17 14:30:00'),
(2, 'ORD-2024-002', 149.99, 'completed', '2024-01-16 11:30:00', '2024-01-18 16:45:00'),
(3, 'ORD-2024-003', 599.98, 'processing', '2024-01-19 09:15:00', NULL),
(4, 'ORD-2024-004', 499.99, 'completed', '2024-01-17 14:20:00', '2024-01-19 10:30:00'),
(5, 'ORD-2024-005', 199.98, 'pending', NULL, NULL);

INSERT INTO order_items (order_id, product_id, quantity, unit_price) VALUES
(1, 1, 1, 299.99),
(2, 2, 1, 149.99),
(3, 1, 1, 299.99),
(3, 4, 3, 99.99),
(4, 3, 1, 499.99),
(5, 4, 2, 99.99);

-- Create production-like views and reports
CREATE VIEW monthly_revenue AS
SELECT 
    DATE_FORMAT(created_at, '%Y-%m') as month,
    COUNT(*) as order_count,
    SUM(total_amount) as total_revenue,
    AVG(total_amount) as avg_order_value
FROM orders 
WHERE status = 'completed'
GROUP BY DATE_FORMAT(created_at, '%Y-%m')
ORDER BY month DESC;

CREATE VIEW product_performance AS
SELECT 
    p.sku,
    p.name,
    p.category,
    COUNT(oi.id) as times_ordered,
    SUM(oi.quantity) as total_quantity_sold,
    SUM(oi.quantity * oi.unit_price) as total_revenue
FROM products p
LEFT JOIN order_items oi ON p.id = oi.product_id
LEFT JOIN orders o ON oi.order_id = o.id AND o.status = 'completed'
GROUP BY p.id, p.sku, p.name, p.category
ORDER BY total_revenue DESC;

-- Grant permissions (more restrictive for prod-like environment)
GRANT SELECT ON prod_database.* TO 'prod_user'@'%';
GRANT INSERT, UPDATE ON prod_database.orders TO 'prod_user'@'%';
GRANT INSERT, UPDATE ON prod_database.order_items TO 'prod_user'@'%';
FLUSH PRIVILEGES;