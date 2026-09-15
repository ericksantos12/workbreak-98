function formatSeconds(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;

  return [hours, minutes, seconds]
    .map((part) => part.toString().padStart(2, "0"))
    .join(":");
}

export function formatElapsedTime(milliseconds: number): string {
  return formatSeconds(Math.floor(Math.max(0, milliseconds) / 1_000));
}

export function formatRemainingTime(milliseconds: number): string {
  return formatSeconds(Math.ceil(Math.max(0, milliseconds) / 1_000));
}
