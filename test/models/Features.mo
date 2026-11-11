package Features "The rules of the layout: own connectors, inheritance, unconnected parts, Diagram frames"
  model Filter "Own connectors without a place: they get one in the icon and in the diagram"
    Parts.RealInput u;
    Parts.RealOutput y;
    Parts.FirstOrder stage1(T = 0.1);
    Parts.FirstOrder stage2(T = 0.2);
    Parts.Gain amplify(k = 3);
  equation
    connect(u, stage1.u);
    connect(stage1.y, stage2.u);
    connect(stage2.y, amplify.u);
    connect(amplify.y, y);
  end Filter;

  model PlacedConnectors "Own connectors with a place in the icon: it becomes the iconTransformation"
    Parts.RealInput u annotation(Placement(transformation(extent = {{-120, 40}, {-80, 80}})));
    Parts.RealOutput y annotation(Placement(transformation(extent = {{100, -70}, {120, -50}})));
    Parts.Integrator integrator;
  equation
    connect(u, integrator.u);
    connect(integrator.y, y);
  end PlacedConnectors;

  model Base "Laid out in the same run as Derived"
    Parts.Constant source(k = 2);
    Parts.Gain gain;
  equation
    connect(source.y, gain.u);
  end Base;

  model Derived "The inherited components sit where Base has them"
    extends Base;
    Parts.Integrator integrator;
    Parts.Gain amplifier;
  equation
    connect(gain.y, integrator.u);
    connect(integrator.y, amplifier.u);
  end Derived;

  model Unconnected "Connected parts in a row, unconnected ones in a grid"
    Parts.Sine sine;
    Parts.Gain gain;
    Parts.Constant spare1;
    Parts.Constant spare2;
    Parts.Gain spare3;
    Parts.Resistor spare4;
    Parts.Inertia spare5;
  equation
    connect(sine.y, gain.u);
  end Unconnected;

  model Framed "An old layout with a frame and its title around two components, and a junction marker"
    Parts.Sine sine annotation(Placement(transformation(extent = {{-90, 10}, {-70, 30}})));
    Parts.Gain gain1 annotation(Placement(transformation(extent = {{-40, 10}, {-20, 30}})));
    Parts.Gain gain2 annotation(Placement(transformation(extent = {{-10, 10}, {10, 30}})));
    Parts.Integrator integrator annotation(Placement(transformation(extent = {{40, 10}, {60, 30}})));
    Parts.Gain sensor annotation(Placement(transformation(extent = {{40, -40}, {60, -20}})));
  equation
    connect(sine.y, gain1.u);
    connect(gain1.y, gain2.u);
    connect(gain2.y, integrator.u);
    connect(gain2.y, sensor.u);
    annotation(Diagram(graphics = {
      Rectangle(extent = {{-50, 40}, {20, 0}}, lineColor = {255, 0, 0}, pattern = LinePattern.Dash),
      Text(extent = {{-50, 50}, {20, 42}}, textString = "gains"),
      Ellipse(extent = {{24, 22}, {28, 18}}, fillPattern = FillPattern.Solid)}));
  end Framed;

  model Interface "Only placed connectors and no connections: left as it is"
    Parts.RealInput a annotation(Placement(transformation(extent = {{-120, -10}, {-100, 10}})));
    Parts.RealOutput b annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
  end Interface;
end Features;
