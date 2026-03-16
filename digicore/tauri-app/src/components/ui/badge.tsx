import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const badgeVariants = cva(
    "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-[var(--dc-accent)] focus:ring-offset-2",
    {
        variants: {
            variant: {
                default:
                    "border-transparent bg-[var(--dc-accent)] text-white hover:bg-[var(--dc-accent-hover)]",
                secondary:
                    "border-transparent bg-[var(--dc-bg-alt)] text-[var(--dc-text)] hover:bg-[var(--dc-bg-tertiary)]",
                destructive:
                    "border-transparent bg-[var(--dc-error)] text-white hover:opacity-90",
                outline: "text-[var(--dc-text)] border-[var(--dc-border)]",
            },
        },
        defaultVariants: {
            variant: "default",
        },
    }
);

export interface BadgeProps
    extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> { }

function Badge({ className, variant, ...props }: BadgeProps) {
    return (
        <div className={cn(badgeVariants({ variant }), className)} {...props} />
    );
}

export { Badge, badgeVariants };
