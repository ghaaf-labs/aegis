UPDATE portfolios
SET name = 'Main portfolio'
WHERE name = 'DAO Reserve';

UPDATE strategies
SET
    name = 'Operating Reserve',
    description = 'Multi-currency reserve for an internet-native organization with multi-jurisdiction operating expenses. USDC + EURC keeps payroll in either denomination; USYC carries the yield sleeve.'
WHERE name = 'DAO Reserve';
