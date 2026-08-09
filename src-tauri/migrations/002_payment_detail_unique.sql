-- Bảng order_payment_detail thiếu UNIQUE index trên order_number, nên
-- `INSERT OR REPLACE` không có gì để "replace" và mỗi lần ghi lại chèn thêm một
-- dòng mới. Gộp dữ liệu trùng (giữ dòng mới nhất) rồi thêm index để upsert đúng.

DELETE FROM order_payment_detail
WHERE id NOT IN (
  SELECT MAX(id) FROM order_payment_detail GROUP BY order_number
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_payment_order_unique
  ON order_payment_detail(order_number);

DROP INDEX IF EXISTS idx_payment_order;

-- orders.has_payment_detail chưa từng được ghi nên đang luôn bằng 0. Dựng lại giá
-- trị đúng từ dữ liệu hiện có.
UPDATE orders SET has_payment_detail = 1
WHERE EXISTS (
  SELECT 1 FROM order_payment_detail
  WHERE order_payment_detail.order_number = orders.order_number
);

UPDATE orders SET has_payment_detail = 0
WHERE has_payment_detail = 1
  AND NOT EXISTS (
    SELECT 1 FROM order_payment_detail
    WHERE order_payment_detail.order_number = orders.order_number
  );

-- Danh sách lệnh luôn được lọc theo trạng thái và sắp theo thời gian tạo giảm dần.
CREATE INDEX IF NOT EXISTS idx_orders_status_create
  ON orders(order_status_code, create_time_ms DESC);

-- Sync tăng dần dò theo update_time_ms.
CREATE INDEX IF NOT EXISTS idx_orders_update
  ON orders(update_time_ms);
