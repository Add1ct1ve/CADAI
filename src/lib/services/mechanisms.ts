import type { MechanismItem } from '$lib/types';

export function buildMechanismInsertPrompt(mechanism: MechanismItem): string {
  const paramLines = mechanism.parameters.map((p) => {
    const unit = p.unit ? ` ${p.unit}` : '';
    return `- ${p.name} = ${p.default_value}${unit}${p.description ? ` (${p.description})` : ''}`;
  });

  return [
    'Use this mechanism profile as a high-priority reference in the generated CAD:',
    '',
    `Mechanism ID: ${mechanism.id}`,
    `Name: ${mechanism.title}`,
    `Category: ${mechanism.category}`,
    `Summary: ${mechanism.summary}`,
    mechanism.keywords.length ? `Keywords: ${mechanism.keywords.join(', ')}` : '',
    '',
    'Mechanism design guidance:',
    mechanism.prompt_block,
    '',
    paramLines.length ? 'Default parameters:' : '',
    ...paramLines,
    '',
    'Preserve parametric variables and robust boolean ordering.',
  ]
    .filter(Boolean)
    .join('\n');
}
