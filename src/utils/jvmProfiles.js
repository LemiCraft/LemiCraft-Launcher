// Ready sets of memory and Java flags for the machines players have. The memory follows the size of the machine,
// the flags are for the Java of a modern client (G1 and its limits); the player can still edit both by hand.

const clamp = (value, min, max) => Math.max(min, Math.min(max, value));

export const JVM_PROFILES = [
  {
    id: 'low',
    label: 'Слабый ПК',
    hint: 'Меньше памяти и потоков сборщика мусора: игра меньше занимает ПК и реже подтормаживает на слабом процессоре',
    args: '-XX:+UseG1GC -XX:MaxGCPauseMillis=100 -XX:+DisableExplicitGC -XX:ParallelGCThreads=2 -XX:ConcGCThreads=1',
    ram: (total) => clamp(Math.floor(total / 2), 2, 3),
  },
  {
    id: 'normal',
    label: 'Обычный',
    hint: 'Подходит большинству: немного модов, средний ПК',
    args: '-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions',
    ram: (total) => clamp(Math.floor(total / 2), 2, 4),
  },
  {
    id: 'heavy',
    label: 'Много модов',
    hint: 'Больше памяти и короткие паузы сборщика: для большой сборки с шейдерами и ресурспаками',
    args:
      '-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=50 -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=16M -XX:G1ReservePercent=20 -XX:+DisableExplicitGC',
    ram: (total) => clamp(Math.floor(total / 2), 6, 10),
  },
];

/** The memory a profile asks for on a machine of `totalGb` */
export function profileRam(profile, totalGb) {
  return profile.ram(Math.max(1, totalGb));
}

/** The profile the machine suits: up to 6 GB the light one, up to 12 the usual, more the heavy one */
export function recommendedProfile(totalGb) {
  if (totalGb <= 6) return JVM_PROFILES[0];
  if (totalGb <= 12) return JVM_PROFILES[1];
  return JVM_PROFILES[2];
}

/** Which profile the current memory and flags are, or null when they are the player's own */
export function profileOf(ram, args, totalGb) {
  const flags = (args ?? '').trim().replace(/\s+/g, ' ');
  return JVM_PROFILES.find((profile) => profile.args === flags && profileRam(profile, totalGb) === Number(ram)) ?? null;
}
