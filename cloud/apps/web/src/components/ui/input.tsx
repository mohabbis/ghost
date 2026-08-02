import * as React from "react";
import { cn } from "@/lib/cn";

const field =
  "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 text-sm outline-none placeholder:text-[var(--color-muted)] focus-visible:border-[var(--color-accent)] disabled:opacity-50";

export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input ref={ref} className={cn(field, "h-9", className)} {...props} />
  ),
);
Input.displayName = "Input";

export const Select = React.forwardRef<
  HTMLSelectElement,
  React.SelectHTMLAttributes<HTMLSelectElement>
>(({ className, ...props }, ref) => (
  <select ref={ref} className={cn(field, "h-9", className)} {...props} />
));
Select.displayName = "Select";

/** Small caps label for a field inside a step form. */
export function FieldLabel({ className, ...props }: React.LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label
      className={cn("mb-1 block text-xs font-medium text-[var(--color-muted)]", className)}
      {...props}
    />
  );
}
