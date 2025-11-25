-- Add additional payment detail fields
-- Migration: 002_add_payment_fields

ALTER TABLE order_payment_detail ADD COLUMN amount TEXT;
ALTER TABLE order_payment_detail ADD COLUMN transfer_content TEXT;
ALTER TABLE order_payment_detail ADD COLUMN suggested_transfer_content TEXT;
