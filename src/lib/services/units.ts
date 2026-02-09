export type UnitSystem = 'mm' | 'inch';

const MM_PER_INCH = 25.4;

export function toDisplay(valueMm: number, units: UnitSystem): number {
  return units === 'inch' ? valueMm / MM_PER_INCH : valueMm;
}

export function fromDisplay(value: number, units: UnitSystem): number {
  return units === 'inch' ? value * MM_PER_INCH : value;
}

export function unitSuffix(units: UnitSystem): string {
  return units === 'inch' ? 'in' : 'mm';
}

export function formatUnit(valueMm: number, units: UnitSystem, decimals = 3): string {
  const display = toDisplay(valueMm, units);
  return `${display.toFixed(decimals)} ${unitSuffix(units)}`;
}

export function areaUnitSuffix(units: UnitSystem): string {
  return units === 'inch' ? 'in\u00B2' : 'mm\u00B2';
}

export function volumeUnitSuffix(units: UnitSystem): string {
  return units === 'inch' ? 'in\u00B3' : 'mm\u00B3';
}

export function toDisplayArea(valueMm2: number, units: UnitSystem): number {
  return units === 'inch' ? valueMm2 / (MM_PER_INCH * MM_PER_INCH) : valueMm2;
}

export function toDisplayVolume(valueMm3: number, units: UnitSystem): number {
  return units === 'inch' ? valueMm3 / (MM_PER_INCH * MM_PER_INCH * MM_PER_INCH) : valueMm3;
}
