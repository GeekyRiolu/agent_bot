import { Skeleton } from "@/components/ui/skeleton";

export function LayoutSkeleton() {
  return (
    <div className="flex h-dvh w-full">
      {/* Sidebar skeleton — hidden on mobile, matches default sidebar width */}
      <div className="hidden w-[--sidebar-width] shrink-0 flex-col border-r bg-sidebar md:flex">
        {/* Sidebar header */}
        <div className="flex items-center justify-between px-4 py-3">
          <Skeleton className="h-6 w-20 rounded-md" />
          <div className="flex gap-1">
            <Skeleton className="size-8 rounded-md" />
            <Skeleton className="size-8 rounded-md" />
          </div>
        </div>
        {/* Sidebar history items */}
        <div className="flex flex-1 flex-col gap-2 px-3 py-2">
          <Skeleton className="h-4 w-16 rounded-md" />
          <Skeleton className="h-8 w-full rounded-md" />
          <Skeleton className="h-8 w-full rounded-md" />
          <Skeleton className="h-8 w-full rounded-md" />
          <Skeleton className="mt-4 h-4 w-24 rounded-md" />
          <Skeleton className="h-8 w-full rounded-md" />
          <Skeleton className="h-8 w-full rounded-md" />
        </div>
        {/* Sidebar footer */}
        <div className="border-t px-3 py-3">
          <Skeleton className="h-8 w-full rounded-md" />
        </div>
      </div>

      {/* Main content area skeleton */}
      <div className="flex min-w-0 flex-1 flex-col">
        <NewChatSkeleton />
      </div>
    </div>
  );
}

export function ChatSkeleton() {
  return (
    <div className="flex h-dvh min-w-0 flex-col bg-background">
      {/* Header skeleton */}
      <header className="sticky top-0 flex items-center gap-2 bg-background px-2 py-1.5 md:px-2">
        <Skeleton className="h-8 w-8 rounded-md" />
        <Skeleton className="h-8 w-24 rounded-md" />
        <Skeleton className="ml-auto h-8 w-28 rounded-md md:ml-0" />
      </header>

      {/* Messages area skeleton */}
      <div className="relative flex-1">
        <div className="absolute inset-0 overflow-y-hidden">
          <div className="mx-auto flex min-w-0 max-w-4xl flex-col gap-6 px-2 py-4 md:px-4">
            {/* User message skeleton */}
            <div className="flex flex-row gap-3 self-end">
              <div className="flex max-w-[80%] flex-col gap-2">
                <Skeleton className="h-10 w-48 rounded-2xl" />
              </div>
            </div>

            {/* Assistant message skeleton */}
            <div className="flex flex-row gap-3">
              <Skeleton className="size-8 shrink-0 rounded-full" />
              <div className="flex max-w-[80%] flex-col gap-2">
                <Skeleton className="h-4 w-72 rounded-lg" />
                <Skeleton className="h-4 w-96 rounded-lg" />
                <Skeleton className="h-4 w-64 rounded-lg" />
              </div>
            </div>

            {/* User message skeleton */}
            <div className="flex flex-row gap-3 self-end">
              <div className="flex max-w-[80%] flex-col gap-2">
                <Skeleton className="h-10 w-64 rounded-2xl" />
              </div>
            </div>

            {/* Assistant message skeleton */}
            <div className="flex flex-row gap-3">
              <Skeleton className="size-8 shrink-0 rounded-full" />
              <div className="flex max-w-[80%] flex-col gap-2">
                <Skeleton className="h-4 w-80 rounded-lg" />
                <Skeleton className="h-4 w-96 rounded-lg" />
                <Skeleton className="h-4 w-48 rounded-lg" />
                <Skeleton className="h-4 w-72 rounded-lg" />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Input area skeleton */}
      <div className="sticky bottom-0 z-1 mx-auto flex w-full max-w-4xl gap-2 border-t-0 bg-background px-2 pb-3 md:px-4 md:pb-4">
        <div className="relative flex w-full flex-col gap-4">
          <Skeleton className="h-[88px] w-full rounded-xl" />
        </div>
      </div>
    </div>
  );
}

export function NewChatSkeleton() {
  return (
    <div className="flex h-dvh min-w-0 flex-col bg-background">
      {/* Header skeleton */}
      <header className="sticky top-0 flex items-center gap-2 bg-background px-2 py-1.5 md:px-2">
        <Skeleton className="h-8 w-8 rounded-md" />
        <Skeleton className="h-8 w-24 rounded-md" />
        <Skeleton className="ml-auto h-8 w-28 rounded-md md:ml-0" />
      </header>

      {/* Greeting area skeleton */}
      <div className="relative flex-1">
        <div className="absolute inset-0 overflow-y-hidden">
          <div className="mx-auto mt-4 flex max-w-3xl flex-col gap-3 px-4 md:mt-16 md:px-8">
            <Skeleton className="h-7 w-56 rounded-lg md:h-8" />
            <Skeleton className="h-7 w-96 rounded-lg md:h-8" />
          </div>
        </div>
      </div>

      {/* Input area skeleton */}
      <div className="sticky bottom-0 z-1 mx-auto flex w-full max-w-4xl gap-2 border-t-0 bg-background px-2 pb-3 md:px-4 md:pb-4">
        <div className="relative flex w-full flex-col gap-4">
          {/* Suggested actions skeleton */}
          <div className="grid gap-2 sm:grid-cols-2">
            <Skeleton className="h-14 w-full rounded-xl" />
            <Skeleton className="h-14 w-full rounded-xl" />
            <Skeleton className="hidden h-14 w-full rounded-xl sm:block" />
            <Skeleton className="hidden h-14 w-full rounded-xl sm:block" />
          </div>
          <Skeleton className="h-[88px] w-full rounded-xl" />
        </div>
      </div>
    </div>
  );
}
