import type {
  BundleSuggestion,
  ChargeType,
  Expense,
  ExpenseType,
  ItemBundle,
  ItemType,
  Party,
  ShopSettings,
} from '@/types';

const SEED_TS = '2026-11-01T00:00:00Z';

/** Mock bundle: same shape as the Rust ItemBundle entity. */
export type MockBundle = ItemBundle;

/** Mock item: Rust Item entity plus mock-only internals (cost, bundles). */
export interface MockItem {
  id: number;
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  itemType: ItemType;
  mrp: number | null;
  defaultSalePrice: number;
  currentStock: number;
  isLowStock: boolean;
  lowStockThreshold: number;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
  costPrice: number;
  bundles: MockBundle[];
}

/** Mock party: Rust Party entity plus fixture ledger stats (PartyListRow / PartyDueSummary). */
export interface MockParty extends Party {
  balanceDue: number;
  oldestBillDate: string | null;
  unpaidBillsCount: number;
}

function makeBundle(
  id: number,
  itemId: number,
  name: string,
  units: number,
  salePrice?: number,
  mrp?: number
): MockBundle {
  return {
    id,
    itemId,
    bundleName: name,
    unitsPerBundle: units,
    mrp: mrp ?? null,
    defaultBundlePrice: salePrice ?? null,
    barcode: `890103099${id}`,
  };
}

function makeItem(
  id: number,
  name: string,
  category: string,
  barcode: string,
  baseUnit: string,
  costPrice: number,
  salePrice: number,
  mrp: number,
  stock: number,
  bundles: MockBundle[] = []
): MockItem {
  return {
    id,
    name,
    category,
    barcode,
    baseUnit,
    itemType: 'PHYSICAL',
    costPrice,
    defaultSalePrice: salePrice,
    mrp,
    currentStock: stock,
    isLowStock: stock < 5,
    lowStockThreshold: 5,
    isActive: true,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
    bundles,
  };
}

export const initialItems: MockItem[] = [
  // 1. Pens & Pencils (26 Items: 1001 - 1026)
  makeItem(1001, 'Apsara Platinum Extra Dark Pencil', 'Pens & Pencils', '8901030381001', 'PCS', 4.5, 6.0, 7.0, 150, [
    makeBundle(1, 1001, 'Box of 10', 10, 55.0, 70.0),
    makeBundle(2, 1001, 'Carton of 100', 100, 500.0, 700.0),
  ]),
  makeItem(1002, 'Nataraj 621 Bold Pencil', 'Pens & Pencils', '8901030381002', 'PCS', 3.5, 5.0, 5.0, 200, [
    makeBundle(3, 1002, 'Box of 10', 10, 45.0, 50.0),
  ]),
  makeItem(1003, 'Cello Butterflow Classic Ball Pen (Blue)', 'Pens & Pencils', '8901030381003', 'PCS', 7.5, 10.0, 10.0, 120, [
    makeBundle(4, 1003, 'Pack of 5', 5, 48.0, 50.0),
    makeBundle(5, 1003, 'Box of 20', 20, 180.0, 200.0),
  ]),
  makeItem(1004, 'Cello Butterflow Classic Ball Pen (Black)', 'Pens & Pencils', '8901030381004', 'PCS', 7.5, 10.0, 10.0, 80, [
    makeBundle(6, 1004, 'Pack of 5', 5, 48.0, 50.0),
    makeBundle(7, 1004, 'Box of 20', 20, 180.0, 200.0),
  ]),
  makeItem(1005, 'Cello Butterflow Classic Ball Pen (Red)', 'Pens & Pencils', '8901030381005', 'PCS', 7.5, 10.0, 10.0, 40, [
    makeBundle(8, 1005, 'Pack of 5', 5, 48.0, 50.0),
  ]),
  makeItem(1006, 'Cello Gripper Ball Pen (Blue)', 'Pens & Pencils', '8901030381006', 'PCS', 8.0, 10.0, 10.0, 100, [
    makeBundle(9, 1006, 'Pack of 5', 5, 48.0, 50.0),
    makeBundle(10, 1006, 'Box of 25', 25, 230.0, 250.0),
  ]),
  makeItem(1007, 'Reynolds 045 Fine Carabine Ball Pen (Blue)', 'Pens & Pencils', '8901030381007', 'PCS', 6.5, 8.0, 10.0, 160, [
    makeBundle(11, 1007, 'Pack of 5', 5, 38.0, 50.0),
    makeBundle(12, 1007, 'Box of 20', 20, 150.0, 200.0),
  ]),
  makeItem(1008, 'Reynolds 045 Fine Carabine Ball Pen (Black)', 'Pens & Pencils', '8901030381008', 'PCS', 6.5, 8.0, 10.0, 90, [
    makeBundle(13, 1008, 'Pack of 5', 5, 38.0, 50.0),
  ]),
  makeItem(1009, 'Reynolds Trimax Liquid Gel Pen (Blue)', 'Pens & Pencils', '8901030381009', 'PCS', 45.0, 55.0, 60.0, 35, [
    makeBundle(14, 1009, 'Pack of 3', 3, 160.0, 180.0),
  ]),
  makeItem(1010, 'Reynolds Trimax Liquid Gel Pen (Black)', 'Pens & Pencils', '8901030381010', 'PCS', 45.0, 55.0, 60.0, 25, [
    makeBundle(15, 1010, 'Pack of 3', 3, 160.0, 180.0),
  ]),
  makeItem(1011, 'Reynolds Trimax Refill (Blue)', 'Pens & Pencils', '8901030381011', 'PCS', 18.0, 25.0, 25.0, 3), // Low Stock
  makeItem(1012, 'Reynolds Trimax Refill (Black)', 'Pens & Pencils', '8901030381012', 'PCS', 18.0, 25.0, 25.0, 20),
  makeItem(1013, 'Uni-ball Eye Rollerball Pen UB-150 (Blue)', 'Pens & Pencils', '8901030381013', 'PCS', 68.0, 85.0, 90.0, 30, [
    makeBundle(16, 1013, 'Pack of 3', 3, 250.0, 270.0),
  ]),
  makeItem(1014, 'Uni-ball Eye Rollerball Pen UB-150 (Black)', 'Pens & Pencils', '8901030381014', 'PCS', 68.0, 85.0, 90.0, 20),
  makeItem(1015, 'Pilot V5 Hi-Tecpoint Pure Liquid Ink Pen (Blue)', 'Pens & Pencils', '8901030381015', 'PCS', 42.0, 50.0, 55.0, 45),
  makeItem(1016, 'Pilot V7 Hi-Tecpoint Pure Liquid Ink Pen (Blue)', 'Pens & Pencils', '8901030381016', 'PCS', 45.0, 55.0, 60.0, 0), // Out of Stock
  makeItem(1017, 'Parker Vector Standard CT Ballpoint Pen', 'Pens & Pencils', '8901030381017', 'PCS', 240.0, 300.0, 350.0, 8),
  makeItem(1018, 'Parker Vector Gold Trim Rollerball Pen', 'Pens & Pencils', '8901030381018', 'PCS', 380.0, 475.0, 550.0, 2), // Low Stock
  makeItem(1019, 'Cross Classic Century Ballpoint Pen', 'Pens & Pencils', '8901030381019', 'PCS', 1800.0, 2200.0, 2500.0, 0), // Out of Stock
  makeItem(1020, 'Pentel GraphGear 500 Mechanical Pencil 0.5mm', 'Pens & Pencils', '8901030381020', 'PCS', 280.0, 350.0, 380.0, 6),
  makeItem(1021, 'Camlin Kokuyo 0.5mm Leads (Tube of 20)', 'Pens & Pencils', '8901030381021', 'PKT', 12.0, 18.0, 20.0, 50, [
    makeBundle(17, 1021, 'Box of 10', 10, 160.0, 200.0),
  ]),
  makeItem(1022, 'Faber-Castell Textliner 38 Highlighter (Yellow)', 'Pens & Pencils', '8901030381022', 'PCS', 20.0, 25.0, 30.0, 60, [
    makeBundle(18, 1022, 'Pack of 5', 5, 120.0, 150.0),
  ]),
  makeItem(1023, 'Faber-Castell Textliner 38 Highlighter (Green)', 'Pens & Pencils', '8901030381023', 'PCS', 20.0, 25.0, 30.0, 40),
  makeItem(1024, 'Camlin Whiteboard Marker (Black)', 'Pens & Pencils', '8901030381024', 'PCS', 22.0, 30.0, 35.0, 55, [
    makeBundle(19, 1024, 'Pack of 4', 4, 110.0, 140.0),
  ]),
  makeItem(1025, 'Camlin Whiteboard Marker (Blue)', 'Pens & Pencils', '8901030381025', 'PCS', 22.0, 30.0, 35.0, 50),
  makeItem(1026, 'Camlin Permanent Marker (Black)', 'Pens & Pencils', '8901030381026', 'PCS', 18.0, 25.0, 25.0, 45),

  // 2. Notebooks & Registers (24 Items: 1027 - 1050)
  makeItem(1027, 'Classmate Long Notebook Softcover Ruled 160 pgs', 'Notebooks & Registers', '8901030381027', 'PCS', 42.0, 55.0, 60.0, 140, [
    makeBundle(20, 1027, 'Pack of 6', 6, 310.0, 360.0),
  ]),
  makeItem(1028, 'Classmate Long Notebook Softcover Ruled 240 pgs', 'Notebooks & Registers', '8901030381028', 'PCS', 60.0, 75.0, 80.0, 90, [
    makeBundle(21, 1028, 'Pack of 6', 6, 430.0, 480.0),
  ]),
  makeItem(1029, 'Classmate Long Notebook Softcover Unruled 160 pgs', 'Notebooks & Registers', '8901030381029', 'PCS', 42.0, 55.0, 60.0, 65),
  makeItem(1030, 'Classmate Regular Exercise Notebook 172 pgs (Single Line)', 'Notebooks & Registers', '8901030381030', 'PCS', 26.0, 35.0, 40.0, 180, [
    makeBundle(22, 1030, 'Pack of 6', 6, 200.0, 240.0),
  ]),
  makeItem(1031, 'Classmate Regular Exercise Notebook 172 pgs (Four Line)', 'Notebooks & Registers', '8901030381031', 'PCS', 26.0, 35.0, 40.0, 110),
  makeItem(1032, 'Classmate Regular Exercise Notebook 172 pgs (Square Grid)', 'Notebooks & Registers', '8901030381032', 'PCS', 26.0, 35.0, 40.0, 85),
  makeItem(1033, 'Classmate Pulse Spiral Notebook A5 160 pgs', 'Notebooks & Registers', '8901030381033', 'PCS', 65.0, 85.0, 95.0, 40),
  makeItem(1034, 'Classmate Pulse Spiral Notebook A4 300 pgs (5-Subject)', 'Notebooks & Registers', '8901030381034', 'PCS', 145.0, 185.0, 200.0, 30),
  makeItem(1035, 'Classmate Practical Hardbound Journal 192 pgs', 'Notebooks & Registers', '8901030381035', 'PCS', 95.0, 125.0, 140.0, 2), // Low Stock
  makeItem(1036, 'Classmate Drawing Book 40 Pages A4', 'Notebooks & Registers', '8901030381036', 'PCS', 32.0, 45.0, 50.0, 70),
  makeItem(1037, 'Classmate Drawing Book 40 Pages A3', 'Notebooks & Registers', '8901030381037', 'PCS', 60.0, 80.0, 90.0, 0), // Out of Stock
  makeItem(1038, 'Neelgagan Hardbound Accounts Register No. 2 (192 pgs)', 'Notebooks & Registers', '8901030381038', 'PCS', 85.0, 110.0, 125.0, 25),
  makeItem(1039, 'Neelgagan Hardbound Accounts Register No. 4 (384 pgs)', 'Notebooks & Registers', '8901030381039', 'PCS', 150.0, 190.0, 210.0, 18),
  makeItem(1040, 'Neelgagan Hardbound Accounts Register No. 6 (576 pgs)', 'Notebooks & Registers', '8901030381040', 'PCS', 210.0, 270.0, 300.0, 12),
  makeItem(1041, 'Neelgagan Attendance Register 60 Workers', 'Notebooks & Registers', '8901030381041', 'PCS', 75.0, 100.0, 110.0, 22),
  makeItem(1042, 'Neelgagan Cash Book Single Column 192 pgs', 'Notebooks & Registers', '8901030381042', 'PCS', 95.0, 125.0, 140.0, 15),
  makeItem(1043, 'Neelgagan Cash Book Double Column 192 pgs', 'Notebooks & Registers', '8901030381043', 'PCS', 105.0, 135.0, 150.0, 14),
  makeItem(1044, 'Neelgagan General Ledger Hardbound 192 pgs', 'Notebooks & Registers', '8901030381044', 'PCS', 110.0, 140.0, 160.0, 16),
  makeItem(1045, 'Neelgagan Duplicate Bill Order Book 1/8 size', 'Notebooks & Registers', '8901030381045', 'PCS', 40.0, 55.0, 60.0, 50, [
    makeBundle(23, 1045, 'Pack of 10', 10, 500.0, 600.0),
  ]),
  makeItem(1046, 'Neelgagan Triplicate Bill Book 1/8 size', 'Notebooks & Registers', '8901030381046', 'PCS', 55.0, 75.0, 85.0, 30),
  makeItem(1047, '3M Post-it Canary Yellow Notes 3x3 (100 Sheets)', 'Notebooks & Registers', '8901030381047', 'PKT', 42.0, 55.0, 60.0, 45, [
    makeBundle(24, 1047, 'Pack of 6', 6, 300.0, 360.0),
  ]),
  makeItem(1048, '3M Post-it Colour Notes Cube 3x3 (400 Sheets)', 'Notebooks & Registers', '8901030381048', 'PKT', 160.0, 210.0, 230.0, 18),
  makeItem(1049, '3M Post-it Flags / Page Markers (5 Assorted Colors)', 'Notebooks & Registers', '8901030381049', 'PKT', 75.0, 95.0, 110.0, 35),
  makeItem(1050, 'Pocket Spiral Notepad 3x5 (80 pgs)', 'Notebooks & Registers', '8901030381050', 'PCS', 12.0, 20.0, 20.0, 80, [
    makeBundle(25, 1050, 'Pack of 10', 10, 180.0, 200.0),
  ]),

  // 3. Paper & Printing (20 Items: 1051 - 1070)
  makeItem(1051, 'JK Copier Paper A4 75 GSM (500 Sheets Ream)', 'Paper & Printing', '8901030381051', 'REAM', 260.0, 310.0, 340.0, 95, [
    makeBundle(26, 1051, 'Box of 5 Reams', 5, 1500.0, 1700.0),
    makeBundle(27, 1051, 'Carton of 10 Reams', 10, 2950.0, 3400.0),
  ]),
  makeItem(1052, 'JK Easy Copier Paper A4 70 GSM (500 Sheets Ream)', 'Paper & Printing', '8901030381052', 'REAM', 235.0, 280.0, 310.0, 80, [
    makeBundle(28, 1052, 'Box of 5 Reams', 5, 1370.0, 1550.0),
  ]),
  makeItem(1053, 'JK Copier Paper FS / Legal 75 GSM (500 Sheets Ream)', 'Paper & Printing', '8901030381053', 'REAM', 320.0, 380.0, 420.0, 40),
  makeItem(1054, 'JK Cedar Copier Paper A3 75 GSM (500 Sheets Ream)', 'Paper & Printing', '8901030381054', 'REAM', 540.0, 650.0, 720.0, 3), // Low Stock
  makeItem(1055, 'BILT Matrix Multipurpose Copier Paper A4 75 GSM', 'Paper & Printing', '8901030381055', 'REAM', 255.0, 305.0, 335.0, 60),
  makeItem(1056, 'Century Green Copier Paper A4 75 GSM', 'Paper & Printing', '8901030381056', 'REAM', 250.0, 300.0, 330.0, 50),
  makeItem(1057, 'Oddy Glossy Photo Paper A4 180 GSM (20 Sheets)', 'Paper & Printing', '8901030381057', 'PKT', 95.0, 130.0, 150.0, 25),
  makeItem(1058, 'Oddy Glossy Photo Paper A4 210 GSM (50 Sheets)', 'Paper & Printing', '8901030381058', 'PKT', 240.0, 310.0, 350.0, 18),
  makeItem(1059, 'Oddy Self-Adhesive Printable Labels A4 24-Up (100 Sheets)', 'Paper & Printing', '8901030381059', 'PKT', 280.0, 360.0, 400.0, 15),
  makeItem(1060, 'Kores Sapphire Blue Carbon Paper (Box of 100 Sheets)', 'Paper & Printing', '8901030381060', 'BOX', 175.0, 230.0, 260.0, 20),
  makeItem(1061, 'Kores Black Carbon Paper (Box of 100 Sheets)', 'Paper & Printing', '8901030381061', 'BOX', 175.0, 230.0, 260.0, 16),
  makeItem(1062, 'Gateway Natural Tracing Paper A4 85 GSM (50 Sheets)', 'Paper & Printing', '8901030381062', 'PKT', 180.0, 240.0, 270.0, 14),
  makeItem(1063, 'Thermal POS Paper Roll 79mm x 50m', 'Paper & Printing', '8901030381063', 'ROLL', 32.0, 45.0, 50.0, 110, [
    makeBundle(29, 1063, 'Box of 10', 10, 420.0, 500.0),
    makeBundle(30, 1063, 'Box of 50', 50, 1950.0, 2500.0),
  ]),
  makeItem(1064, 'Thermal POS Paper Roll 57mm x 20m (EDC Machine)', 'Paper & Printing', '8901030381064', 'ROLL', 14.0, 22.0, 25.0, 90, [
    makeBundle(31, 1064, 'Box of 10', 10, 200.0, 250.0),
  ]),
  makeItem(1065, 'Bristol Thick White Chart Paper (Full Sheet)', 'Paper & Printing', '8901030381065', 'PCS', 6.0, 10.0, 10.0, 150),
  makeItem(1066, 'Bristol Fluorescent Chart Paper (Assorted Colors)', 'Paper & Printing', '8901030381066', 'PCS', 8.0, 12.0, 15.0, 120),
  makeItem(1067, 'Handmade Marble Paper Sheet (Imperial Size)', 'Paper & Printing', '8901030381067', 'PCS', 18.0, 30.0, 35.0, 35),
  makeItem(1068, 'Brown Craft Wrapping Paper Sheet', 'Paper & Printing', '8901030381068', 'PCS', 4.0, 7.0, 8.0, 200),
  makeItem(1069, 'Butter Paper Roll (5 Metre)', 'Paper & Printing', '8901030381069', 'ROLL', 38.0, 55.0, 65.0, 28),
  makeItem(1070, 'Millimetre Graph Paper Pad A4 (100 Sheets)', 'Paper & Printing', '8901030381070', 'PKT', 40.0, 60.0, 70.0, 45),

  // 4. Adhesives & Tapes (20 Items: 1071 - 1090)
  makeItem(1071, 'Fevicol MR Squeeze Bottle 22.5g', 'Adhesives & Tapes', '8901030381071', 'PCS', 8.0, 10.0, 10.0, 120, [
    makeBundle(32, 1071, 'Box of 20', 20, 190.0, 200.0),
  ]),
  makeItem(1072, 'Fevicol MR Squeeze Bottle 50g', 'Adhesives & Tapes', '8901030381072', 'PCS', 16.0, 20.0, 20.0, 85),
  makeItem(1073, 'Fevicol MR Squeeze Bottle 100g', 'Adhesives & Tapes', '8901030381073', 'PCS', 32.0, 40.0, 40.0, 50),
  makeItem(1074, 'Fevicol MR Squeeze Bottle 200g', 'Adhesives & Tapes', '8901030381074', 'PCS', 58.0, 75.0, 75.0, 0), // Out of Stock
  makeItem(1075, 'Fevistik PVA Glue Stick 8g', 'Adhesives & Tapes', '8901030381075', 'PCS', 12.0, 15.0, 15.0, 140, [
    makeBundle(33, 1075, 'Box of 30', 30, 420.0, 450.0),
  ]),
  makeItem(1076, 'Fevistik PVA Glue Stick 15g', 'Adhesives & Tapes', '8901030381076', 'PCS', 20.0, 25.0, 25.0, 90, [
    makeBundle(34, 1076, 'Box of 20', 20, 470.0, 500.0),
  ]),
  makeItem(1077, 'Fevistik PVA Glue Stick 22g', 'Adhesives & Tapes', '8901030381077', 'PCS', 32.0, 40.0, 40.0, 60),
  makeItem(1078, 'Fevikwik Instant Adhesive 1g', 'Adhesives & Tapes', '8901030381078', 'PCS', 4.0, 5.0, 5.0, 300, [
    makeBundle(35, 1078, 'Card of 24', 24, 110.0, 120.0),
  ]),
  makeItem(1079, 'Fevikwik Gel 3g Anti-Clog Pack', 'Adhesives & Tapes', '8901030381079', 'PCS', 24.0, 30.0, 30.0, 40),
  makeItem(1080, 'Wonder 555 Clear Cello Tape 1/2 inch x 25m', 'Adhesives & Tapes', '8901030381080', 'ROLL', 6.0, 10.0, 10.0, 180, [
    makeBundle(36, 1080, 'Box of 12', 12, 105.0, 120.0),
  ]),
  makeItem(1081, 'Wonder 555 Clear Cello Tape 1 inch x 30m', 'Adhesives & Tapes', '8901030381081', 'ROLL', 14.0, 20.0, 25.0, 100, [
    makeBundle(37, 1081, 'Box of 8', 8, 150.0, 200.0),
  ]),
  makeItem(1082, 'Brown Packaging Box Tape 2 inch x 50m', 'Adhesives & Tapes', '8901030381082', 'ROLL', 38.0, 55.0, 65.0, 70, [
    makeBundle(38, 1082, 'Box of 6', 6, 300.0, 390.0),
  ]),
  makeItem(1083, 'Transparent Packaging Tape 2 inch x 50m', 'Adhesives & Tapes', '8901030381083', 'ROLL', 38.0, 55.0, 65.0, 65, [
    makeBundle(39, 1083, 'Box of 6', 6, 300.0, 390.0),
  ]),
  makeItem(1084, 'Abro Paper Masking Tape 1 inch x 20m', 'Adhesives & Tapes', '8901030381084', 'ROLL', 25.0, 35.0, 40.0, 50),
  makeItem(1085, 'Abro Paper Masking Tape 2 inch x 20m', 'Adhesives & Tapes', '8901030381085', 'ROLL', 48.0, 65.0, 75.0, 30),
  makeItem(1086, 'Scotch Double Sided Foam Tape 1 inch x 3m', 'Adhesives & Tapes', '8901030381086', 'ROLL', 55.0, 75.0, 85.0, 25),
  makeItem(1087, 'Scotch Double Sided Tissue Tape 1/2 inch x 10m', 'Adhesives & Tapes', '8901030381087', 'ROLL', 22.0, 35.0, 40.0, 40),
  makeItem(1088, 'Scotch Heavy Duty Packaging Tape Dispenser', 'Adhesives & Tapes', '8901030381088', 'PCS', 140.0, 185.0, 220.0, 3), // Low Stock
  makeItem(1089, 'Fevicryl Fabric Glue 30ml Bottle', 'Adhesives & Tapes', '8901030381089', 'PCS', 18.0, 25.0, 25.0, 45),
  makeItem(1090, 'Pidilite Glue Drops Removable Adhesive Dots (Pack of 64)', 'Adhesives & Tapes', '8901030381090', 'PKT', 30.0, 40.0, 45.0, 35),

  // 5. Files & Folders (20 Items: 1091 - 1110)
  makeItem(1091, 'Solo Lever Arch File Heavy Duty A4 (75mm)', 'Files & Folders', '8901030381091', 'PCS', 125.0, 165.0, 185.0, 40, [
    makeBundle(40, 1091, 'Box of 10', 10, 1550.0, 1850.0),
  ]),
  makeItem(1092, 'Solo Lever Arch File FS / Legal (75mm)', 'Files & Folders', '8901030381092', 'PCS', 135.0, 175.0, 200.0, 35),
  makeItem(1093, 'Solo 2-Ring Binder File A4 (25mm)', 'Files & Folders', '8901030381093', 'PCS', 85.0, 115.0, 130.0, 50),
  makeItem(1094, 'Solo Cobra Spring File Deluxe Plastic FS', 'Files & Folders', '8901030381094', 'PCS', 35.0, 50.0, 55.0, 80, [
    makeBundle(41, 1094, 'Pack of 10', 10, 470.0, 550.0),
  ]),
  makeItem(1095, 'Standard Cardboard Cobra Spring File (Office Green)', 'Files & Folders', '8901030381095', 'PCS', 16.0, 25.0, 30.0, 150, [
    makeBundle(42, 1095, 'Pack of 10', 10, 230.0, 300.0),
  ]),
  makeItem(1096, 'Solo Display Book 20 Pockets A4', 'Files & Folders', '8901030381096', 'PCS', 65.0, 90.0, 105.0, 35),
  makeItem(1097, 'Solo Display Book 40 Pockets A4', 'Files & Folders', '8901030381097', 'PCS', 110.0, 145.0, 165.0, 25),
  makeItem(1098, 'Solo Display Book 60 Pockets A4', 'Files & Folders', '8901030381098', 'PCS', 155.0, 200.0, 230.0, 4), // Low Stock
  makeItem(1099, 'Polypropylene Clear L-Folder A4 Document Sleeve', 'Files & Folders', '8901030381099', 'PCS', 4.5, 8.0, 10.0, 300, [
    makeBundle(43, 1099, 'Pack of 10', 10, 75.0, 100.0),
    makeBundle(44, 1099, 'Box of 50', 50, 350.0, 500.0),
  ]),
  makeItem(1100, 'Polypropylene Button Folder Envelope A4', 'Files & Folders', '8901030381100', 'PCS', 8.0, 15.0, 18.0, 220, [
    makeBundle(45, 1100, 'Pack of 10', 10, 135.0, 180.0),
  ]),
  makeItem(1101, 'Polypropylene Button Folder Envelope FS / Legal', 'Files & Folders', '8901030381101', 'PCS', 11.0, 20.0, 22.0, 160, [
    makeBundle(46, 1101, 'Pack of 10', 10, 180.0, 220.0),
  ]),
  makeItem(1102, 'Expanding Accordion File 12 Pockets with Handle A4', 'Files & Folders', '8901030381102', 'PCS', 185.0, 240.0, 275.0, 15),
  makeItem(1103, 'Solo Clip Board / Exam Pad Plastic A4', 'Files & Folders', '8901030381103', 'PCS', 65.0, 90.0, 100.0, 40),
  makeItem(1104, 'Wooden Hardboard Exam Pad A4', 'Files & Folders', '8901030381104', 'PCS', 35.0, 50.0, 60.0, 60),
  makeItem(1105, 'Solo Plastic Index Divider Tabs 1-12 A4', 'Files & Folders', '8901030381105', 'SET', 45.0, 65.0, 75.0, 30),
  makeItem(1106, 'Solo Plastic Index Divider Tabs Monthly Jan-Dec A4', 'Files & Folders', '8901030381106', 'SET', 45.0, 65.0, 75.0, 25),
  makeItem(1107, 'Clear Report Cover Slide Binder File A4', 'Files & Folders', '8901030381107', 'PCS', 7.0, 12.0, 15.0, 180, [
    makeBundle(47, 1107, 'Pack of 10', 10, 110.0, 150.0),
  ]),
  makeItem(1108, 'Strip File / Project File Plastic Transparent A4', 'Files & Folders', '8901030381108', 'PCS', 6.0, 10.0, 12.0, 200, [
    makeBundle(48, 1108, 'Pack of 10', 10, 90.0, 120.0),
  ]),
  makeItem(1109, 'Cardboard Flat File with Cotton Lace Tag', 'Files & Folders', '8901030381109', 'PCS', 5.0, 8.0, 10.0, 250, [
    makeBundle(49, 1109, 'Pack of 20', 20, 150.0, 200.0),
  ]),
  makeItem(1110, 'Cotton File Lace / Tag 18 inch (Bundle of 100)', 'Files & Folders', '8901030381110', 'PKT', 25.0, 40.0, 50.0, 50),

  // 6. Geometry & Math Instruments (18 Items: 1111 - 1128)
  makeItem(1111, 'Camlin Scholar Geometry Box', 'Geometry & Math Instruments', '8901030381111', 'BOX', 95.0, 125.0, 140.0, 60),
  makeItem(1112, 'Camlin Exam Mathematical Geometry Box Tin', 'Geometry & Math Instruments', '8901030381112', 'BOX', 75.0, 100.0, 110.0, 75),
  makeItem(1113, 'Faber-Castell Math Geometry Box Tin Case', 'Geometry & Math Instruments', '8901030381113', 'BOX', 110.0, 145.0, 160.0, 40),
  makeItem(1114, 'Camlin Transparent Plastic Scale Ruler 15cm', 'Geometry & Math Instruments', '8901030381114', 'PCS', 4.0, 6.0, 7.0, 250, [
    makeBundle(50, 1114, 'Box of 20', 20, 110.0, 140.0),
  ]),
  makeItem(1115, 'Camlin Transparent Plastic Scale Ruler 30cm', 'Geometry & Math Instruments', '8901030381115', 'PCS', 9.0, 15.0, 15.0, 160, [
    makeBundle(51, 1115, 'Box of 20', 20, 270.0, 300.0),
  ]),
  makeItem(1116, 'Stainless Steel Scale Ruler 15cm (Engineering Grade)', 'Geometry & Math Instruments', '8901030381116', 'PCS', 18.0, 28.0, 35.0, 45),
  makeItem(1117, 'Stainless Steel Scale Ruler 30cm (Engineering Grade)', 'Geometry & Math Instruments', '8901030381117', 'PCS', 32.0, 50.0, 60.0, 50),
  makeItem(1118, 'Stainless Steel Scale Ruler 60cm', 'Geometry & Math Instruments', '8901030381118', 'PCS', 85.0, 125.0, 150.0, 15),
  makeItem(1119, 'Maped Kidz Universal Compass with Pencil', 'Geometry & Math Instruments', '8901030381119', 'PCS', 35.0, 50.0, 55.0, 40),
  makeItem(1120, 'Precision Die-Cast Technical Drawing Compass', 'Geometry & Math Instruments', '8901030381120', 'PCS', 120.0, 165.0, 190.0, 20),
  makeItem(1121, 'Omega Mini Drafter for Engineering Drawing', 'Geometry & Math Instruments', '8901030381121', 'PCS', 360.0, 460.0, 520.0, 12),
  makeItem(1122, 'Omega Set Squares 45 & 60 deg 10 inch Pair', 'Geometry & Math Instruments', '8901030381122', 'SET', 70.0, 95.0, 110.0, 25),
  makeItem(1123, 'Transparent 360 Full Circle Protractor', 'Geometry & Math Instruments', '8901030381123', 'PCS', 15.0, 25.0, 30.0, 40),
  makeItem(1124, 'Transparent 180 Half Circle Protractor', 'Geometry & Math Instruments', '8901030381124', 'PCS', 5.0, 10.0, 10.0, 120),
  makeItem(1125, 'Flexible French Curves Set of 3 Acrylic', 'Geometry & Math Instruments', '8901030381125', 'SET', 55.0, 80.0, 95.0, 25),
  makeItem(1126, 'Flexible Curve Ruler 50cm Vinyl', 'Geometry & Math Instruments', '8901030381126', 'PCS', 80.0, 115.0, 135.0, 16),
  makeItem(1127, 'Casio FX-82MS Scientific Calculator', 'Geometry & Math Instruments', '8901030381127', 'PCS', 520.0, 610.0, 675.0, 15),
  makeItem(1128, 'Casio FX-991CW ClassWiz Scientific Calculator', 'Geometry & Math Instruments', '8901030381128', 'PCS', 1150.0, 1375.0, 1595.0, 1), // Low Stock

  // 7. Art & Craft Supplies (22 Items: 1129 - 1150)
  makeItem(1129, 'Camlin Oil Pastels Set of 15 Shades', 'Art & Craft Supplies', '8901030381129', 'PKT', 38.0, 50.0, 55.0, 80),
  makeItem(1130, 'Camlin Oil Pastels Set of 25 Shades Plastic Case', 'Art & Craft Supplies', '8901030381130', 'BOX', 85.0, 110.0, 125.0, 50),
  makeItem(1131, 'Camlin Oil Pastels Set of 50 Shades with Scraping Tool', 'Art & Craft Supplies', '8901030381131', 'BOX', 185.0, 240.0, 275.0, 25),
  makeItem(1132, 'Faber-Castell Triangular Wax Crayons 12 Shades', 'Art & Craft Supplies', '8901030381132', 'PKT', 32.0, 45.0, 50.0, 70),
  makeItem(1133, 'Faber-Castell Plastic Crayons 24 Shades Tin Box', 'Art & Craft Supplies', '8901030381133', 'BOX', 115.0, 150.0, 175.0, 35),
  makeItem(1134, 'Camlin Water Colour Cakes 18 Shades with Brush', 'Art & Craft Supplies', '8901030381134', 'BOX', 65.0, 85.0, 95.0, 55),
  makeItem(1135, 'Camlin Water Colour Tubes 12 Shades (5ml)', 'Art & Craft Supplies', '8901030381135', 'BOX', 85.0, 110.0, 125.0, 40),
  makeItem(1136, 'Camlin Artist Acrylic Colour 12 Tubes Set', 'Art & Craft Supplies', '8901030381136', 'BOX', 210.0, 270.0, 300.0, 4), // Low Stock
  makeItem(1137, 'Faber-Castell Connector Sketch Pens 15 Shades', 'Art & Craft Supplies', '8901030381137', 'PKT', 65.0, 85.0, 95.0, 60),
  makeItem(1138, 'Faber-Castell Connector Sketch Pens 25 Shades', 'Art & Craft Supplies', '8901030381138', 'PKT', 110.0, 145.0, 160.0, 40),
  makeItem(1139, 'Camlin Series 66 Round Paintbrush Set of 4 (Sizes 2, 4, 6, 8)', 'Art & Craft Supplies', '8901030381139', 'SET', 85.0, 115.0, 130.0, 30),
  makeItem(1140, 'Camlin Series 67 Flat Paintbrush Set of 4 (Sizes 2, 4, 6, 8)', 'Art & Craft Supplies', '8901030381140', 'SET', 95.0, 130.0, 145.0, 28),
  makeItem(1141, 'Fine Art Fan Paintbrush Size 4', 'Art & Craft Supplies', '8901030381141', 'PCS', 40.0, 60.0, 70.0, 20),
  makeItem(1142, 'White Plastic Kidney Mixing Palette 10 Wells', 'Art & Craft Supplies', '8901030381142', 'PCS', 22.0, 35.0, 40.0, 50),
  makeItem(1143, 'Camlin Kokuyo Fun Dough Modelling Clay 6 Tubs', 'Art & Craft Supplies', '8901030381143', 'BOX', 95.0, 130.0, 150.0, 30),
  makeItem(1144, 'Faber-Castell Polychromos Colour Pencil 12 Tin', 'Art & Craft Supplies', '8901030381144', 'BOX', 920.0, 1150.0, 1250.0, 1), // Low Stock
  makeItem(1145, 'Camlin Charcoal Drawing Pencils (Set of 3)', 'Art & Craft Supplies', '8901030381145', 'PKT', 45.0, 65.0, 75.0, 35),
  makeItem(1146, 'Camlin Graphite Drawing Pencils Set of 6 (2B to 8B)', 'Art & Craft Supplies', '8901030381146', 'PKT', 60.0, 80.0, 90.0, 45),
  makeItem(1147, 'Koh-I-Noor Kneadable Art Eraser (Grey)', 'Art & Craft Supplies', '8901030381147', 'PCS', 32.0, 45.0, 55.0, 50),
  makeItem(1148, 'Cotton Stretched Artist Canvas Board 8x10 inch', 'Art & Craft Supplies', '8901030381148', 'PCS', 45.0, 65.0, 75.0, 30),
  makeItem(1149, 'Cotton Stretched Artist Canvas Board 10x12 inch', 'Art & Craft Supplies', '8901030381149', 'PCS', 65.0, 90.0, 105.0, 25),
  makeItem(1150, 'Cotton Stretched Artist Canvas Board 12x16 inch', 'Art & Craft Supplies', '8901030381150', 'PCS', 95.0, 130.0, 150.0, 20),

  // 8. Desk & Office Accessories (22 Items: 1151 - 1172)
  makeItem(1151, 'Kangaro No. 10 Stapler (Compact)', 'Desk & Office Accessories', '8901030381151', 'PCS', 45.0, 60.0, 70.0, 65, [
    makeBundle(52, 1151, 'Box of 10', 10, 570.0, 700.0),
  ]),
  makeItem(1152, 'Kangaro HD-10D Stapler with Pin Remover', 'Desk & Office Accessories', '8901030381152', 'PCS', 75.0, 100.0, 115.0, 40),
  makeItem(1153, 'Kangaro HD-45 Desktop Office Stapler (Heavy Duty 24/6)', 'Desk & Office Accessories', '8901030381153', 'PCS', 180.0, 235.0, 265.0, 25),
  makeItem(1154, 'Kangaro HD-23S17 Giant Heavy Duty Stapler', 'Desk & Office Accessories', '8901030381154', 'PCS', 850.0, 1050.0, 1200.0, 2), // Low Stock
  makeItem(1155, 'Kangaro No. 10-1M Staple Pins (1000 Pins Box)', 'Desk & Office Accessories', '8901030381155', 'BOX', 7.0, 10.0, 10.0, 350, [
    makeBundle(53, 1155, 'Pack of 10 Boxes', 10, 95.0, 100.0),
    makeBundle(54, 1155, 'Carton of 100', 100, 900.0, 1000.0),
  ]),
  makeItem(1156, 'Kangaro 24/6 Staple Pins (1000 Pins Box)', 'Desk & Office Accessories', '8901030381156', 'BOX', 18.0, 25.0, 28.0, 150, [
    makeBundle(55, 1156, 'Pack of 10 Boxes', 10, 230.0, 280.0),
  ]),
  makeItem(1157, 'Kangaro Two-Hole Punch DP-52', 'Desk & Office Accessories', '8901030381157', 'PCS', 110.0, 150.0, 170.0, 0), // Out of Stock
  makeItem(1158, 'Kangaro Two-Hole Heavy Punch DP-600', 'Desk & Office Accessories', '8901030381158', 'PCS', 210.0, 275.0, 310.0, 18),
  makeItem(1159, 'Kangaro Single Hole Punch 1-Hole Plier', 'Desk & Office Accessories', '8901030381159', 'PCS', 65.0, 90.0, 100.0, 30),
  makeItem(1160, 'Nataraj Plasto Eraser (Medium)', 'Desk & Office Accessories', '8901030381160', 'PCS', 2.5, 4.0, 5.0, 400, [
    makeBundle(56, 1160, 'Box of 20', 20, 75.0, 100.0),
  ]),
  makeItem(1161, 'Apsara Non-Dust Eraser Jumbo', 'Desk & Office Accessories', '8901030381161', 'PCS', 4.5, 7.0, 8.0, 250, [
    makeBundle(57, 1161, 'Box of 20', 20, 130.0, 160.0),
  ]),
  makeItem(1162, 'Camlin Exam Pencil Sharpener Table Container', 'Desk & Office Accessories', '8901030381162', 'PCS', 3.0, 5.0, 5.0, 300, [
    makeBundle(58, 1162, 'Jar of 50', 50, 230.0, 250.0),
  ]),
  makeItem(1163, 'Rotary Tabletop Mechanical Pencil Sharpener Machine', 'Desk & Office Accessories', '8901030381163', 'PCS', 165.0, 220.0, 250.0, 12),
  makeItem(1164, 'Kangaroo Binder Clips 19mm 3/4 inch (Box of 12)', 'Desk & Office Accessories', '8901030381164', 'BOX', 30.0, 45.0, 50.0, 90, [
    makeBundle(59, 1164, 'Dozen Pack', 12, 45.0, 50.0),
  ]),
  makeItem(1165, 'Kangaroo Binder Clips 25mm 1 inch (Box of 12)', 'Desk & Office Accessories', '8901030381165', 'BOX', 42.0, 60.0, 70.0, 70, [
    makeBundle(60, 1165, 'Dozen Pack', 12, 60.0, 70.0),
  ]),
  makeItem(1166, 'Kangaroo Binder Clips 32mm 1.25 inch (Box of 12)', 'Desk & Office Accessories', '8901030381166', 'BOX', 65.0, 90.0, 105.0, 45),
  makeItem(1167, 'Kangaroo Binder Clips 51mm 2 inch (Box of 12)', 'Desk & Office Accessories', '8901030381167', 'BOX', 110.0, 150.0, 175.0, 25),
  makeItem(1168, 'Gem Plastic Coated Colour Paper Clips (Box of 100)', 'Desk & Office Accessories', '8901030381168', 'BOX', 15.0, 25.0, 30.0, 110, [
    makeBundle(61, 1168, 'Pack of 10 Boxes', 10, 230.0, 300.0),
  ]),
  makeItem(1169, 'Metal Paper Pins U-Clip T-Pins (Box of 100g)', 'Desk & Office Accessories', '8901030381169', 'BOX', 25.0, 40.0, 45.0, 60),
  makeItem(1170, 'SDI Craft Utility Knife Paper Cutter 9mm Small', 'Desk & Office Accessories', '8901030381170', 'PCS', 18.0, 28.0, 35.0, 70),
  makeItem(1171, 'SDI Craft Utility Knife Paper Cutter 18mm Heavy', 'Desk & Office Accessories', '8901030381171', 'PCS', 45.0, 65.0, 75.0, 40),
  makeItem(1172, 'Kangaroo Stainless Steel Scissors 8.5 inch Office', 'Desk & Office Accessories', '8901030381172', 'PCS', 75.0, 110.0, 125.0, 35),
];

export const initialCustomers: MockParty[] = [
  {
    id: 1,
    name: 'Delhi Public School (Stationery Dept)',
    phone: '9811023456',
    partyType: 'CUSTOMER',
    balanceDue: 4500.0,
    oldestBillDate: '2026-10-15',
    unpaidBillsCount: 2,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 2,
    name: 'Apex Coaching Classes',
    phone: '9823456789',
    partyType: 'CUSTOMER',
    balanceDue: 1850.0,
    oldestBillDate: '2026-11-01',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 3,
    name: 'Shree Ram Associates (CA Office)',
    phone: '9871122334',
    partyType: 'CUSTOMER',
    balanceDue: 750.0,
    oldestBillDate: '2026-11-10',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 4,
    name: 'Ramesh Kumar (Regular Student)',
    phone: '9899112233',
    partyType: 'CUSTOMER',
    balanceDue: 220.0,
    oldestBillDate: '2026-11-15',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 5,
    name: 'Sunbeam English Academy',
    phone: '9810987654',
    partyType: 'CUSTOMER',
    balanceDue: 3200.0,
    oldestBillDate: '2026-10-28',
    unpaidBillsCount: 2,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 6,
    name: 'Walk-in Retail Cash Customer',
    phone: '9800000000',
    partyType: 'CUSTOMER',
    balanceDue: 0.0,
    oldestBillDate: null,
    unpaidBillsCount: 0,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
];

export const initialVendors: MockParty[] = [
  {
    id: 101,
    name: 'Hindustan Paper & Stationery Mart',
    phone: '9811234567',
    partyType: 'VENDOR',
    balanceDue: 12500.0,
    oldestBillDate: '2026-10-10',
    unpaidBillsCount: 2,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 102,
    name: 'ITC Paperboards & Stationery Distributors',
    phone: '9822334455',
    partyType: 'VENDOR',
    balanceDue: 8400.0,
    oldestBillDate: '2026-10-20',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 103,
    name: 'Kangaro Tools & Fasteners Agency',
    phone: '9833445566',
    partyType: 'VENDOR',
    balanceDue: 3100.0,
    oldestBillDate: '2026-11-05',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 104,
    name: 'Camlin Kokuyo Regional Depot',
    phone: '9844556677',
    partyType: 'VENDOR',
    balanceDue: 5600.0,
    oldestBillDate: '2026-10-18',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
  {
    id: 105,
    name: 'Pidilite Adhesives Wholesale Supplier',
    phone: '9855667788',
    partyType: 'VENDOR',
    balanceDue: 1800.0,
    oldestBillDate: '2026-11-02',
    unpaidBillsCount: 1,
    isActive: true,
    notes: null,
    createdAt: SEED_TS,
    updatedAt: SEED_TS,
  },
];

export const initialShopSettings: ShopSettings = {
  id: 1,
  shopName: 'Aggarwal Stationery & Book Store',
  phone: '+91 98123 45678',
  address: 'Shop 14, Central Market, Lajpat Nagar, New Delhi - 110024',
  taxId: '07AAAAA0000A1Z5',
};

export const initialBundleSuggestions: BundleSuggestion[] = [
  {
    bundleName: 'Box of 10',
    unitsPerBundle: 10,
    usageCount: 42,
  },
  {
    bundleName: 'Pack of 5',
    unitsPerBundle: 5,
    usageCount: 35,
  },
  {
    bundleName: 'Box of 20',
    unitsPerBundle: 20,
    usageCount: 28,
  },
  {
    bundleName: 'Dozen Pack',
    unitsPerBundle: 12,
    usageCount: 19,
  },
  {
    bundleName: 'Pack of 6',
    unitsPerBundle: 6,
    usageCount: 15,
  },
  {
    bundleName: 'Box of 5 Reams',
    unitsPerBundle: 5,
    usageCount: 12,
  },
  {
    bundleName: 'Carton of 100',
    unitsPerBundle: 100,
    usageCount: 9,
  },
];

export const initialChargeTypes: ChargeType[] = [
  { id: 1, name: 'Transport', effect: 'CHARGE', appliesTo: 'BOTH', defaultAmount: 50, isActive: true, createdAt: '2026-11-01T09:00:00Z' },
  { id: 2, name: 'Delivery', effect: 'CHARGE', appliesTo: 'SALE', defaultAmount: 30, isActive: true, createdAt: '2026-11-01T09:00:00Z' },
  { id: 3, name: 'Cash Discount', effect: 'DISCOUNT', appliesTo: 'BOTH', defaultAmount: null, isActive: true, createdAt: '2026-11-01T09:00:00Z' },
  { id: 4, name: 'Carrier Fee', effect: 'CHARGE', appliesTo: 'PURCHASE', defaultAmount: 100, isActive: true, createdAt: '2026-11-01T09:00:00Z' },
];

export const initialExpenseTypes: ExpenseType[] = [
  { id: 1, name: 'Printer Ink', isActive: true, createdAt: '2026-11-01T09:00:00Z' },
  { id: 2, name: 'Electricity Bill', isActive: true, createdAt: '2026-11-01T09:00:00Z' },
  { id: 3, name: 'Tea & Snacks', isActive: true, createdAt: '2026-11-01T09:00:00Z' },
];

/** Five expenses in the current month (dates relative to `monthDay`, e.g. `(d) => '2026-11-0' + d`). */
export function initialExpenses(monthDay: (day: number) => string): Expense[] {
  const row = (id: number, day: number, typeId: number, amount: number, notes: string | null): Expense => ({
    id,
    expenseCode: `EXP-${1000 + id}`,
    expenseDate: monthDay(day),
    expenseTypeId: typeId,
    expenseTypeName: initialExpenseTypes.find((t) => t.id === typeId)?.name ?? '',
    amount,
    notes,
    createdAt: `${monthDay(day)}T09:00:00Z`,
    updatedAt: null,
  });
  return [
    row(1, 1, 1, 450, 'Black cartridge'),
    row(2, 2, 3, 120, null),
    row(3, 3, 2, 1800, 'October bill'),
    row(4, 4, 3, 80, null),
    row(5, 5, 1, 300, null),
  ];
}
