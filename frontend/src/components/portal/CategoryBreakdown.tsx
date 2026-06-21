import { type CSSProperties } from 'react';
import type { RequirementCompleteness } from '@/types/portal';
import './CategoryBreakdown.css';

interface CategoryBreakdownProps {
  /** Per-requirement completeness entries to group by category. */
  readonly byRequirement: readonly RequirementCompleteness[];
}

interface CategoryGroup {
  readonly name: string;
  readonly status: 'verified' | 'pending' | 'failed';
}

function deriveCategoryFromClaimType(claimType: string): string {
  if (claimType.includes('entity')) return 'entity';
  if (claimType.includes('signer') || claimType.includes('authorized')) return 'signer';
  if (claimType.includes('jurisdiction') || claimType.includes('region')) return 'region';
  if (claimType.includes('wallet')) return 'wallet';
  if (claimType.includes('financial') || claimType.includes('threshold')) return 'financial';
  if (claimType.includes('compliance')) return 'compliance';
  return 'custom';
}

function groupByCategory(items: readonly RequirementCompleteness[]): readonly CategoryGroup[] {
  const categoryMap = new Map<string, 'verified' | 'pending' | 'failed'>();

  for (const item of items) {
    const category = deriveCategoryFromClaimType(item.claim_type);
    const existing = categoryMap.get(category);

    if (item.status === 'failed') {
      categoryMap.set(category, 'failed');
    } else if (item.status === 'verified' && existing !== 'failed') {
      categoryMap.set(category, 'verified');
    } else if (!existing) {
      categoryMap.set(category, 'pending');
    }
  }

  return Array.from(categoryMap.entries()).map(([name, status]) => ({ name, status }));
}

/**
 * Compact vertical list showing category-level verification status.
 * Each row: colored dot + category name + status label.
 */
export function CategoryBreakdown({ byRequirement }: CategoryBreakdownProps) {
  const categories = groupByCategory(byRequirement);

  if (categories.length === 0) {
    return null;
  }

  const statusSymbol: Record<string, string> = {
    verified: '✓',
    pending: '○',
    failed: '✗',
  };

  return (
    <div className="category-breakdown">
      <h4 className="category-breakdown__title">Categories</h4>
      {categories.map((cat, index) => {
        const delayStyle: CSSProperties = {
          animationDelay: `${index * 60}ms`,
        };

        return (
          <div
            key={cat.name}
            className="category-breakdown__item"
            style={delayStyle}
          >
            <span className={`category-breakdown__dot category-breakdown__dot--${cat.status}`} />
            <span className="category-breakdown__name">{cat.name}</span>
            <span className={`category-breakdown__status category-breakdown__status--${cat.status}`}>
              {statusSymbol[cat.status]} {cat.status}
            </span>
          </div>
        );
      })}
    </div>
  );
}
