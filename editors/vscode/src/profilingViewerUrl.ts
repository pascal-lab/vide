export function buildSpeedscopeUrl(viewerUrl: string, profileUrl: string, title: string): string {
  const params = new URLSearchParams({
    profileURL: profileUrl,
    title,
  });
  return `${viewerUrl}#${params.toString()}`;
}
