yooo.... so you want to use sim??

# simulator

ok first things first you need to install the simulator. you can do that by running the following command in your terminal (use git-bash if ur on windows):

```sh
curl -fsSL https://raw.githubusercontent.com/cornellev/ev-sim/main/install.sh | bash
```

then say yes (y) to pretty much everything that it asks you.

then, once it's done running, you may have to run the following command if you're on windows to get nvm to work:

```sh
touch ~/.bashrc
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
source ~/.bashrc
```

then, you can run the following commands to start the simulator:

```sh
npm run build
```

then:

```
npm start
```

give it a minute to startup, and wait until it prints the logo and a message that says that it's ready on http://[some ip or localhost]:3000. then, open that url in your browser.

once it's open, it can look a little confusing at first. to get started, we need to set it up.

press escape or press the button in the top right corner to open the workspace menu, and then click on the bag symbol on the left side of the menu. this will open the marketplace workspace.

then, at the top bar, click on the "library" button. click on "import package" and then click on "choose file."

find the `onboarding.run-template.tar` in the `environment` folder of this repo, and select it. then confirm to import it. once it's done, you can click the "Open run configuration" button.

Then, this will open up the run configuration. what the run configuration is, is basically some setup to actually run the simulation, which details the world, the robot, the sensors, and more. click on the "validate" button at the top bar. it should say that the configuration is valid; if it doesn't, then let me know.

if you want to view your vehicle, you can find it within the "vehicle editor" workspace from the menu from before. it should pop up automatically, if not click on the name by the vehicle editor workspace button and a menu should pop up.

to see your environment, you can click on the "environment editor" workspace from the menu. it should pop up automatically, but if not, click on the dropdown at the top bar and you should be able to change the environment to the "minicar environment"

once everything is set up, you should be able to launch the simulation from this repo! make sure you run the setup first to check that everything is valid, and then you should be good to go from there.

if you're running sim and want to make it less annoying to see, click on the eye and then click "diagonistics" to disable the sensor markers and the origin marker. you can also disable the massive steering arc in the visibility menu > roads & trajectories.

if you want to see your occupancy grid, enable visibility > occupancy > imported occupancy. it should appear while you're running the stack, if not then i'll see if i can fix that.


after a run, if you want to inspect it, in the workspace menu go see under the inspect area, offering "replay" or "analysis" or "logs." replay allows you to see the run again (sorta), it might be a bit helpful to see in the 2d view what happened (click on "3d"). analysis can show you the data and graphs of what's happening as well. logs can be used for management, and you can delete your logs if you need. additionally, it might be helpful to see the 2d view during the simulation run, which you can do by clicking on the map icon while the sim is running.

let me know if there are any issues or errors, i'll try to help you out. thank you!!