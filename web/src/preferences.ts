export const readStoredPreference = <T extends string>(
  storage: Pick<Storage, 'getItem'> | undefined,
  key: string,
  fallback: T,
): T => {
  const stored = storage?.getItem(key) as T | null | undefined
  return stored ?? fallback
}
