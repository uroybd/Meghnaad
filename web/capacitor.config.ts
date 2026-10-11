import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'me.utsob.meghnaad',
  appName: 'Meghnaad',
  // The same build as the web app: the engine in the Android app is native, and the page picks it up.
  webDir: 'dist',
  android: { path: '../android' },
  server: { androidScheme: 'https' },
};

export default config;
