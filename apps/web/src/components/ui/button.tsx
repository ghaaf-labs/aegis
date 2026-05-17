import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

// FE-SHAD-1 (phase 1): in-place neo-brutalism restyle. Variants now map
// to the @aegis/ui semantic tones (pnl/agent/neutral/ghost/danger) so
// every `<Button>` import inherits the design-system contract without a
// call-site migration. Phase 2 deletes this file in favour of importing
// BrutalButton directly from @aegis/ui.

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap text-sm font-semibold border-brutal border-black rounded-sharp transition-[box-shadow,transform] duration-100 active:translate-y-px focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400 focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-text-hi text-black hover:shadow-brutal-sm",
        destructive: "bg-risk text-black hover:shadow-brutal-sm",
        outline:
          "bg-transparent text-text-default hover:text-text-hi hover:bg-raised",
        secondary: "bg-raised text-text-hi hover:shadow-brutal-sm",
        ghost:
          "bg-transparent border-transparent text-text-default hover:text-text-hi hover:bg-raised",
        link: "border-transparent text-accent-agent underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 px-3 text-xs",
        lg: "h-11 px-8",
        icon: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

interface ButtonProps
  extends
    React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button };
