const { spawn } = require('child_process');
const readline = require('readline');

// Create readline interface for user input
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout
});

// Function to execute a Tauri command
function executeTauriCommand(command, args = []) {
  console.log(`Running command: ${command} with args: ${JSON.stringify(args)}`);

  // Build the tauri CLI command
  const tauriProcess = spawn('tauri', ['invoke', command, ...args.map(arg => JSON.stringify(arg))], {
    cwd: process.cwd(),
    stdio: 'inherit'
  });

  return new Promise((resolve, reject) => {
    tauriProcess.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Command failed with code ${code}`));
      }
    });

    tauriProcess.on('error', (err) => {
      reject(err);
    });
  });
}

// Main menu function
async function showMenu() {
  console.log('\n--- Mouse Coordinate Debug Menu ---');
  console.log('1. Show display information');
  console.log('2. Show current cursor position');
  console.log('3. Test point on display');
  console.log('4. Test mouse click');
  console.log('5. Exit');

  rl.question('Enter your choice: ', async (choice) => {
    try {
      switch (choice) {
        case '1':
          await executeTauriCommand('debug_displays');
          break;

        case '2':
          await executeTauriCommand('debug_cursor');
          break;

        case '3':
          rl.question('Enter X coordinate: ', (x) => {
            rl.question('Enter Y coordinate: ', async (y) => {
              await executeTauriCommand('debug_point', [parseFloat(x), parseFloat(y)]);
              showMenu();
            });
          });
          return; // Skip the automatic showMenu() call

        case '4':
          rl.question('Enter X coordinate: ', (x) => {
            rl.question('Enter Y coordinate: ', async (y) => {
              await executeTauriCommand('debug_click', [parseFloat(x), parseFloat(y)]);
              showMenu();
            });
          });
          return; // Skip the automatic showMenu() call

        case '5':
          console.log('Exiting...');
          rl.close();
          return;

        default:
          console.log('Invalid choice, please try again.');
      }

      // Show menu again for all options except those that have their own input flow
      showMenu();

    } catch (error) {
      console.error('Error:', error.message);
      showMenu();
    }
  });
}

// Start the menu
console.log('Mouse Coordinate Debug Tool');
console.log('---------------------------');
console.log('This tool helps diagnose mouse coordinate system issues.');
showMenu();
