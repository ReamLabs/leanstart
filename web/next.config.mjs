/** @type {import('next').NextConfig} */
const nextConfig = {
  // Logs can be large; allow bigger request bodies on the ingest route.
  experimental: { serverComponentsExternalPackages: ["tar-stream"] },
};
export default nextConfig;
