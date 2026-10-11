import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { installNativeApi, isNative } from './engine/native';

// In the Android app the settings live on the phone, so the page's requests to its server are answered there.
if (isNative()) installNativeApi();

export default mount(App, { target: document.getElementById('app')! });
