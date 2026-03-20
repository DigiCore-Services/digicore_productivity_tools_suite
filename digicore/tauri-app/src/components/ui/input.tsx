import * as React from "react";
import { cn } from "../../lib/utils";

export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        type={type}
        className={cn(
          "flex h-9 w-full rounded-md border border-[var(--dc-border)] bg-[var(--dc-bg)] px-3 py-1 text-sm text-[var(--dc-text)] shadow-sm transition-colors placeholder:text-[var(--dc-text-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--dc-accent)] focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
          className
        )}
        ref={ref}
        {...props}
      />
    );
  }
);
Input.displayName = "Input";

export { Input };
