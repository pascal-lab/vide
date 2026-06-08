import { parsePackageCliArgs } from './package/cli';
import { createPackageContext } from './package/context';
import { packageExtension } from './package/packageExtension';

function main(): void {
  const options = parsePackageCliArgs(process.argv.slice(2));
  const vsixPath = packageExtension(options, createPackageContext(__dirname));
  console.log(vsixPath);
}

main();
