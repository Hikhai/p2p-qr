/**
 * Các hàm định dạng dùng chung.
 *
 * `Intl.NumberFormat` khá tốn kém để khởi tạo, và trước đây mỗi ô trong bảng lại
 * tạo một instance mới ở mỗi lần render. Ở đây khởi tạo một lần rồi dùng lại.
 */

const fiatFormatter = new Intl.NumberFormat('vi-VN', {
  minimumFractionDigits: 0,
  maximumFractionDigits: 0
});

const assetFormatters = new Map<number, Intl.NumberFormat>();

const dateTimeFormatter = new Intl.DateTimeFormat('vi-VN', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit'
});

/** Số chữ số thập phân phù hợp cho từng loại tài sản. */
function assetDigits(asset: string): number {
  switch (asset) {
    case 'USDT':
    case 'USDC':
    case 'BUSD':
      return 2;
    case 'BTC':
      return 6;
    case 'ETH':
      return 4;
    default:
      return 8;
  }
}

function toNumber(value: string | number | null | undefined): number {
  if (value === null || value === undefined || value === '') return 0;
  const num = typeof value === 'string' ? Number(value) : value;
  return Number.isFinite(num) ? num : 0;
}

export function formatFiat(value: string | number | null | undefined): string {
  return fiatFormatter.format(toNumber(value));
}

export function formatAsset(value: string | number | null | undefined, asset: string): string {
  const digits = assetDigits(asset);
  let formatter = assetFormatters.get(digits);
  if (!formatter) {
    formatter = new Intl.NumberFormat('vi-VN', {
      minimumFractionDigits: 0,
      maximumFractionDigits: digits
    });
    assetFormatters.set(digits, formatter);
  }
  return formatter.format(toNumber(value));
}

export function formatDateTime(ms: number | null | undefined): string {
  if (!ms) return '';
  return dateTimeFormatter.format(new Date(ms));
}

export function timeAgo(ms: number | null | undefined): string {
  if (!ms) return 'Chưa bao giờ';
  const seconds = Math.floor((Date.now() - ms) / 1000);
  if (seconds < 60) return `${seconds}s trước`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} phút trước`;
  return `${Math.floor(seconds / 3600)} giờ trước`;
}

/** Đơn giá mỗi đơn vị tài sản, tính lại từ tổng tiền khi API không trả về giá. */
export function pricePerUnit(price: string, totalFiat: string, amountAsset: string): number {
  const direct = toNumber(price);
  if (direct > 0) return direct;

  const total = toNumber(totalFiat);
  const amount = toNumber(amountAsset);
  return total > 0 && amount > 0 ? total / amount : 0;
}

export { toNumber };
