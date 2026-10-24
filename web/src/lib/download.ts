/** Offer `data` to be kept as a file called `name`. */
export function saveAs(name: string, data: Blob | string, mime = 'text/plain'): void {
  const url = URL.createObjectURL(typeof data === 'string' ? new Blob([data], { type: mime }) : data);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
