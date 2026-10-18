// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/codeg/LICENSE.
import { cn } from "../../ui/source-primitives"

function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn("bg-muted rounded-xl animate-pulse", className)}
      {...props}
    />
  )
}

export { Skeleton }
