import * as React from "react";
import { cn } from "../../lib/utils";

interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number;
  max?: number;
  /// Show a sweeping marquee when the total is unknown.
  indeterminate?: boolean;
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({ className, value = 0, max = 100, indeterminate, ...props }, ref) => {
    const pct = Math.max(0, Math.min(100, max > 0 ? (value / max) * 100 : 0));
    return (
      <div
        ref={ref}
        className={cn(
          "relative h-2 w-full overflow-hidden rounded-full bg-[var(--color-accent)]",
          className,
        )}
        {...props}
      >
        {indeterminate ? (
          <div className="absolute inset-y-0 left-0 w-1/3 animate-[progress-indeterminate_1.5s_ease-in-out_infinite] rounded-full bg-[var(--color-primary)]" />
        ) : (
          <div
            className="h-full bg-[var(--color-primary)] transition-all duration-300"
            style={{ width: `${pct}%` }}
          />
        )}
      </div>
    );
  },
);
Progress.displayName = "Progress";

export { Progress };
