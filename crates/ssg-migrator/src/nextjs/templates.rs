//! String templates for Next.js project generation.

pub(super) const USE_ROUTER_TEMPLATE: &str = r#""use client";

import NextLink, { LinkProps } from "next/link";
import { usePathname, useParams, useRouter } from "next/navigation";

const useLocation = () => {
  const pathname = usePathname();
  return { pathname };
};

const useNavigate = () => {
  const router = useRouter();
  return router.push;
};

const Link = ({
  to,
  href,
  ...args
}: Omit<LinkProps, "href"> & {
  to?: string;
  href?: string;
  className?: string;
  children?: React.ReactNode | undefined;
}) => {
  return <NextLink href={href ?? to ?? "/"} {...args} />;
};

export type NavLinkProps = Omit<LinkProps, "href" | "className" | "ref"> & {
  to?: string;
  href?: string;
  ref?: React.Ref<HTMLAnchorElement>;
  className?: string | ((props: { isActive: boolean; isPending: boolean }) => string);
  children?: React.ReactNode;
};

const NavLink = ({
  to,
  href,
  ref,
  className,
  activeClassName,
  pendingClassName,
  children,
  ...args
}: NavLinkProps & {
  activeClassName?: string;
  pendingClassName?: string;
}) => {
  const pathname = usePathname();
  const target = href || to || "/";
  const isActive = pathname === target || (target !== "/" && pathname.startsWith(target + "/"));
  const isPending = false;
  const resolvedClassName =
    typeof className === "function"
      ? className({ isActive, isPending })
      : [className, isActive && activeClassName, isPending && pendingClassName]
          .filter(Boolean)
          .join(" ");
  return (
    <NextLink ref={ref} href={target} className={resolvedClassName} {...args}>
      {children}
    </NextLink>
  );
};

export { Link, NavLink, useLocation, useParams, useNavigate };
"#;

pub(super) const LAYOUT_TEMPLATE: &str = r#"import "@/index.css";
import Providers from "./providers";

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
"#;

pub(super) const PAGE_TEMPLATE: &str = r#"import PAGENAMEPage from "@/pages/PAGENAME";

export default function PAGENAME() {
  return <PAGENAMEPage />;
}
"#;

pub(super) const LOADING_TEMPLATE: &str = r#"export default function Loading() {
  return (
    <div className="flex min-h-screen items-center justify-center">
      <div className="animate-pulse text-muted-foreground">Loading...</div>
    </div>
  );
}
"#;

pub(super) const ERROR_TEMPLATE: &str = r#""use client";

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4">
      <h2 className="text-lg font-semibold">Something went wrong</h2>
      <button
        onClick={() => reset()}
        className="rounded-md border px-4 py-2 text-sm hover:bg-muted"
      >
        Try again
      </button>
    </div>
  );
}
"#;
