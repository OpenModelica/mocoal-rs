model Drive "Speed control of an inertia; text only, no graphical annotations"
  Modelica.Blocks.Sources.Step step(startTime = 0.1);
  Modelica.Blocks.Math.Feedback feedback;
  Modelica.Blocks.Continuous.PI pi(k = 10, T = 0.1);
  Modelica.Mechanics.Rotational.Sources.Torque torque;
  Modelica.Mechanics.Rotational.Components.Inertia inertia(J = 1);
  Modelica.Mechanics.Rotational.Components.Damper damper(d = 0.5);
  Modelica.Mechanics.Rotational.Components.Fixed fixed;
  Modelica.Mechanics.Rotational.Sensors.SpeedSensor speed;
equation
  connect(step.y, feedback.u1);
  connect(feedback.y, pi.u);
  connect(pi.y, torque.tau);
  connect(torque.flange, inertia.flange_a);
  connect(inertia.flange_b, damper.flange_a);
  connect(damper.flange_b, fixed.flange);
  connect(inertia.flange_b, speed.flange);
  connect(speed.w, feedback.u2);
end Drive;
