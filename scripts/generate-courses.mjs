// Maintainer tool: reuse ev-sim authoring, validation, resolution, and sensors.
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
const simRoot = path.resolve(process.argv[2] ?? '../ev-sim');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const load = (file) => import(pathToFileURL(path.join(simRoot, file)));
const { StorageService } = await load('server/storage/StorageService.js');
const { createDefaultRunManifest } = await load('app/simulation/RunManifest.js');
const { canonicalRunBundleStringify, verifyRunBundleIntegrity } = await load('server/headless/RunBundle.js');
const scratch = await mkdtemp(path.join(os.tmpdir(), 'cev-onboarding-courses-'));
const write = async (file, value) => writeFile(path.join(root, file), JSON.stringify(value, null, 2) + '\n');
try {
  const storage = new StorageService(scratch);
  await mkdir(path.join(root, 'scenarios'), { recursive: true });
  await mkdir(path.join(root, 'fixtures'), { recursive: true });
  const vehicleId = 'onboarding-training';
  // Synthetic dimensions, explicitly NOT measurements of the physical minicar.
  const vehicle = await storage.createVehicleManifest({
    id: vehicleId, name: 'Onboarding training vehicle (simulation only)',
    description: 'Synthetic 30 x 18 cm training vehicle. Replace with measured geometry before hardware use.',
    model: { asset: null, scale: 1, rotation: {}, offset: {} },
    boundingBox: { size: { x: .3, y: .2, z: .18 }, center: { x: 0, y: .1, z: 0 } },
    wheels: [-1, 1].flatMap(side => [
      { id: `front-${side}`, position: { x: .1, y: .04, z: side * .07 }, radius: .04, width: .02, steerable: true },
      { id: `rear-${side}`, position: { x: -.1, y: .04, z: side * .07 }, radius: .04, width: .02, steerable: false },
    ]),
    kinematics: { wheelbase: .2, maxSteeringAngle: .5, maxSpeed: .3, maxAcceleration: .5, maxDeceleration: 1, maxJerk: 3, maxSteeringRate: 1.5 },
  });
  const spec = { wheelbase_m: .2, length_m: .3, width_m: .18,
    rear_axle_in_base: { x: -.1, y: 0 }, sensor_in_vehicle: { position: { x: .1, y: 0 }, yaw_rad: 0 },
    max_speed_mps: .3, max_steering_rad: .5, hardware_calibrated: false };
  await write('fixtures/vehicle.json', spec);
  const grid = (occupied = []) => {
    const g = { origin: { x: 0, y: -3 }, resolution_m: .1, width: 100, height: 60, occupied: Array(6000).fill(false) };
    for (const [col, row] of occupied) g.occupied[row * g.width + col] = true;
    return g;
  };
  const controls = [{ x:1,y:0 }, { x:2,y:0 }, { x:4,y:0 }, { x:6,y:0 }, { x:8,y:0 }];
  for (const name of ['empty', 'detour', 'blocked']) {
    const id = `onboarding-${name}`;
    const obstacles = name === 'empty' ? [] : [{ id: 'obstacle', shape: 'box',
      position: { x: 4, y: .5, z: 0 }, rotationY: 0,
      size: { x: .8, y: 1, z: name === 'blocked' ? 8 : .8 }, color: '#cb7c55', tags: ['unknown'] }];
    const env = await storage.createEnvironment({ id, name: `Onboarding ${name}`, initialManifest: {
      roadsAuthored: true, buildingsAuthored: true, featuresAuthored: true,
      document: { environmentId: id, chunkSize: 20, roadsAuthored: true, buildingsAuthored: true, featuresAuthored: true,
        roads: { nodes: [{ id:'a',x:0,z:0 },{ id:'b',x:10,z:0 }], edges: [{ id:'road',startNodeId:'a',endNodeId:'b',width:8,laneCount:1,bidirectional:true }] },
        buildings: [], features: [], primitives: obstacles, earth: null },
    }});
    let scenario = { id, name:`Onboarding ${name}`, environment:{ id:env.environmentId },
      actors:[{ id:'ego',name:'Minicar',role:'ego',vehicleId,enabled:true }],
      routes:[{ id:'ego-route',name:'Task endpoints (not a student path)',actorId:'ego',initialSpeedMps:0,
        // The legacy scenario external-ros path strips stamps and submits reference commands.
        // Leave it disabled and unmatched; candidate /controls/command goes directly to ControlRuntime.
        controller:{ kind:'external-ros',topicId:'onboarding-unused-reference',activation:{kind:'flag',flag:'onboarding.enable-reference'} },
        waypoints:[{ id:'start',order:0,kind:'start',position:{x:1.1,y:0,z:0},heading:0,anchor:{kind:'road',id:'road',fraction:.11} },
          { id:'finish',order:1,kind:'finish',position:{x:8.1,y:0,z:0},heading:0,anchor:{kind:'road',id:'road',fraction:.81} }] }],
      zones:[],triggers:[],completion:{conditions:[{id:'collision',kind:'ego-collision'},{id:'timeout',kind:'max-duration',durationNs:90e9}]},
      expectedOutcomes:[{id:'no-collisions',kind:'no-collisions'}],sensorAliases:[],parameters:[] };
    const verification = await storage.verifyScenarioRoute(id, { scenario, routeId:'ego-route' });
    if (!verification.ok) throw new Error(JSON.stringify(verification));
    scenario.routes[0].waypoints = verification.waypoints;
    scenario.routes[0].verification = verification.verification;
    scenario = await storage.createScenario(scenario);
    const manifest = createDefaultRunManifest();
    Object.assign(manifest, { id, name:`Onboarding ${name}`, description:'No built-in autonomy. Candidate commands only.',
      environment:{id},scenario:{id,expectedHash:scenario.definitionHash,egoVehicleId:vehicleId,sensorBindings:{},parameterValues:{}} });
    manifest.initialState.vehicles = [{id:'ego',type:vehicleId,pose:{position:{x:1.1,y:0,z:0},rotation:{x:0,y:0,z:0,order:'XYZ'}},linearVelocity:{x:0,y:0,z:0},steeringAngle:0}];
    manifest.clock = {...manifest.clock,stepNs:20_000_000,pacing:'realtime',speed:1,maxSteps:4500};
    manifest.controls = {...manifest.controls,authority:'candidate',referenceShadow:false,watchdogNs:300_000_000,stalePolicy:'stop'};
    const lidar = manifest.sensorRig.sensors.find(s => s.type === 'lidar3d');
    Object.assign(lidar, {rateHz:10,pose:{position:{x:0,y:0,z:.3},rotation:{x:0,y:0,z:0,order:'XYZ'}},syncGroupId:null});
    // The manifest requires increasing elevation bounds; this sub-step interval emits one horizontal row.
    lidar.calibration = {range:20,azimuth:{startDeg:-180,endDeg:180,stepDeg:1},elevation:{startDeg:0,endDeg:.000001,stepDeg:1},products:{pointCloud:true,semanticPointCloud:false,diagnostics:true}};
    delete lidar.outputs.semanticPointCloudTopicId;
    lidar.noise = {}; lidar.latency = {};
    manifest.sensorRig.sensors = [lidar];manifest.sensorRig.syncGroups = [];
    const keep = new Set(['clock','tf','tf-static','truth-odometry','front-lidar-points','front-lidar-diagnostics','controls-command']);
    manifest.topics = manifest.topics.filter(t => keep.has(t.id));
    manifest.topics.push({id:'onboarding-unused-reference',name:'/onboarding/unused_reference',direction:'input',schema:{type:'nav_msgs/Odometry',version:1},required:false,producer:'reference',authority:'reference',routeDownstream:false});
    await storage.createRunManifest(manifest);
    const bundle = await storage.exportRunManifest(id);
    verifyRunBundleIntegrity(bundle);
    await writeFile(path.join(root,'scenarios',`${name}.bundle.json`),canonicalRunBundleStringify(bundle));
    const cells = [];
    if (name !== 'empty') for(let row=0;row<60;row++) for(let col=36;col<44;col++) {
      const y=-3+(row+.5)*.1;if(name==='blocked'||Math.abs(y)<.4) cells.push([col,row]);
    }
    const map = grid(cells);
    const course={manifest_id:id,resolved_hash:bundle.resolved.resolvedHash,goal:{x:8,y:0},goal_tolerance_m:.2,timeout_s:90,vehicle:spec,grid:grid(),control_path:controls};
    await write(`scenarios/${name}.json`,course);
    await write(`fixtures/planning-${name}.json`,{grid:map,start:{pose:{position:{x:1,y:0},yaw_rad:0},speed_mps:0},goal:course.goal,vehicle:spec});
    console.log(`${name}: ${bundle.resolved.resolvedHash}`);
  }
  // Small analytically known sensor fixture, not a mapping implementation.
  await write('fixtures/perception.json', {time_ns:100_000_000,state:{pose:{position:{x:1,y:0},yaw_rad:Math.PI/2},speed_mps:0},
    scan:{points:[{x:1,y:0},{x:1,y:0},{x:2,y:-1}],sensor_in_vehicle:{position:{x:.1,y:0},yaw_rad:0}}});
  await write('fixtures/path-curve.json',[{x:1,y:0},{x:2,y:0},{x:3,y:.3},{x:4,y:.8},{x:5,y:1},{x:6,y:.8},{x:7,y:.3},{x:8,y:0}]);
} finally { await rm(scratch,{recursive:true,force:true}); }
